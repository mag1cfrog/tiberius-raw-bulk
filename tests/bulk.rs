use futures_util::io::{AsyncRead, AsyncWrite};
use futures_util::stream::TryStreamExt;
use names::{Generator, Name};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::env;
use std::sync::Once;
use tiberius::{IntoSql, Result, TokenRow};

#[cfg(all(feature = "tds73", feature = "chrono"))]
use chrono::DateTime;
#[cfg(all(feature = "tds73", feature = "chrono"))]
use chrono::NaiveDateTime;

use runtimes_macro::test_on_runtimes;

// This is used in the testing macro :)
#[allow(dead_code)]
static LOGGER_SETUP: Once = Once::new();

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("TIBERIUS_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

static ENCRYPTED_CONN_STR: Lazy<String> = Lazy::new(|| format!("{};encrypt=true", *CONN_STR));

static PLAIN_TEXT_CONN_STR: Lazy<String> =
    Lazy::new(|| format!("{};encrypt=DANGER_PLAINTEXT", *CONN_STR));

thread_local! {
    static NAMES: RefCell<Option<Generator<'static>>> =
    RefCell::new(None);
}

async fn random_table() -> String {
    NAMES.with(|maybe_generator| {
        maybe_generator
            .borrow_mut()
            .get_or_insert_with(|| Generator::with_naming(Name::Plain))
            .next()
            .unwrap()
            .replace('-', "")
    })
}

macro_rules! test_bulk_type {
    ($name:ident($sql_type:literal, $total_generated:expr, $generator:expr)) => {
        paste::item! {
            #[test_on_runtimes]
            async fn [< bulk_load_optional_ $name >]<S>(mut conn: tiberius::Client<S>) -> Result<()>
            where
                S: AsyncRead + AsyncWrite + Unpin + Send,
            {
                let table = format!("##{}", random_table().await);

                conn.execute(
                    &format!(
                        "CREATE TABLE {} (id INT IDENTITY PRIMARY KEY, content {} NULL)",
                        table,
                        $sql_type,
                    ),
                    &[],
                )
                    .await?;

                let mut req = conn.bulk_insert(&table).await?;

                for i in $generator {
                    let mut row = TokenRow::new();
                    row.push(i.into_sql());
                    req.send(row).await?;
                }

                let res = req.finalize().await?;

                assert_eq!($total_generated, res.total());

                Ok(())
            }

            #[test_on_runtimes]
            async fn [< bulk_load_required_ $name >]<S>(mut conn: tiberius::Client<S>) -> Result<()>
            where
                S: AsyncRead + AsyncWrite + Unpin + Send,
            {
                let table = format!("##{}", random_table().await);

                conn.execute(
                    &format!(
                        "CREATE TABLE {} (id INT IDENTITY PRIMARY KEY, content {} NOT NULL)",
                        table,
                        $sql_type
                    ),
                    &[],
                )
                    .await?;

                let mut req = conn.bulk_insert(&table).await?;

                for i in $generator {
                    let mut row = TokenRow::new();
                    row.push(i.into_sql());
                    req.send(row).await?;
                }

                let res = req.finalize().await?;

                assert_eq!($total_generated, res.total());

                Ok(())
            }
        }
    };
}

test_bulk_type!(tinyint("TINYINT", 256, 0..=255u8));
test_bulk_type!(smallint("SMALLINT", 2000, 0..2000i16));
test_bulk_type!(int("INT", 2000, 0..2000i32));
test_bulk_type!(bigint("BIGINT", 2000, 0..2000i64));

#[test_on_runtimes]
async fn bulk_insert_columns_does_not_start_bulk_flow<S>(
    mut conn: tiberius::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.execute(
        &format!("CREATE TABLE {table} (id INT IDENTITY PRIMARY KEY, content INT NOT NULL)"),
        &[],
    )
    .await?;

    let columns = conn.bulk_insert_columns(&table).await?;

    assert_eq!(1, columns.len());
    assert_eq!("content", columns.iter().next().unwrap().name());

    let row = conn
        .query("SELECT 42", &[])
        .await?
        .into_row()
        .await?
        .unwrap();

    assert_eq!(Some(42i32), row.get(0));

    Ok(())
}

#[test_on_runtimes]
async fn bulk_insert_with_columns_sends_rows<S>(mut conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);

    conn.execute(
        &format!("CREATE TABLE {table} (id INT IDENTITY PRIMARY KEY, content INT NOT NULL)"),
        &[],
    )
    .await?;

    let columns = conn.bulk_insert_columns(&table).await?;
    let mut req = conn.bulk_insert_with_columns(&table, columns).await?;

    for value in [7i32, 11i32] {
        let mut row = TokenRow::new();
        row.push(value.into_sql());
        req.send(row).await?;
    }

    let res = req.finalize().await?;

    assert_eq!(2, res.total());

    let values: Vec<i32> = conn
        .query(format!("SELECT content FROM {table} ORDER BY id"), &[])
        .await?
        .try_filter_map(|item| async move { Ok(item.into_row()) })
        .map_ok(|row| row.get::<i32, _>(0).unwrap())
        .try_collect()
        .await?;

    assert_eq!(vec![7, 11], values);

    Ok(())
}

async fn direct_packet_bulk_insert_sends_split_packets<S>(
    mut conn: tiberius::Client<S>,
    expect_tls: bool,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    const ROWS: usize = 64;
    const PAYLOAD_LEN: usize = 2048;

    let table = format!("##{}", random_table().await);

    conn.execute(
        &format!(
            "CREATE TABLE {table} (id INT IDENTITY PRIMARY KEY, content VARBINARY(MAX) NOT NULL)"
        ),
        &[],
    )
    .await?;

    let mut req = conn.bulk_insert(&table).await?;
    req.enable_direct_packet_writes();

    let payloads: Vec<Vec<u8>> = (0..ROWS)
        .map(|value| vec![u8::try_from(value % 251).unwrap(); PAYLOAD_LEN])
        .collect();

    for payload in &payloads {
        let mut row = TokenRow::new();
        row.push(payload.as_slice().into_sql());
        req.send(row).await?;
    }

    let (res, stats) = req.finalize_with_stats().await?;

    assert_eq!(ROWS as u64, res.total());
    assert!(
        stats.packet.packets_written > 0,
        "test payload should force split TDS packets",
    );
    assert!(
        stats.write_timing.direct_packet_write.calls > 1,
        "direct packet writer should see multiple packets",
    );
    if expect_tls {
        assert_eq!(0, stats.write_timing.direct_packet_write.raw_stream_calls);
        assert!(
            stats.write_timing.direct_packet_write.tls_stream_calls > 1,
            "encrypted direct packet writes should use the TLS stream path",
        );
    } else {
        assert!(
            stats.write_timing.direct_packet_write.raw_stream_calls > 1,
            "plaintext direct packet writes should use the raw stream path",
        );
        assert_eq!(0, stats.write_timing.direct_packet_write.tls_stream_calls);
    }

    let row = conn
        .query(
            format!(
                "SELECT COUNT(*), CAST(SUM(DATALENGTH(content)) AS BIGINT), \
                 MIN(DATALENGTH(content)), MAX(DATALENGTH(content)) FROM {table}",
            ),
            &[],
        )
        .await?
        .into_row()
        .await?
        .unwrap();

    assert_eq!(Some(ROWS as i32), row.get(0));
    assert_eq!(Some((ROWS * PAYLOAD_LEN) as i64), row.get(1));
    assert_eq!(Some(PAYLOAD_LEN as i64), row.get(2));
    assert_eq!(Some(PAYLOAD_LEN as i64), row.get(3));

    Ok(())
}

#[test_on_runtimes(connection_string = "PLAIN_TEXT_CONN_STR")]
async fn direct_packet_bulk_insert_sends_split_packets_plaintext<S>(
    conn: tiberius::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    direct_packet_bulk_insert_sends_split_packets(conn, false).await
}

#[test_on_runtimes(connection_string = "ENCRYPTED_CONN_STR")]
async fn direct_packet_bulk_insert_sends_split_packets_encrypted<S>(
    conn: tiberius::Client<S>,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    direct_packet_bulk_insert_sends_split_packets(conn, true).await
}

test_bulk_type!(empty_varchar(
    "VARCHAR(MAX)",
    100,
    vec![""; 100].into_iter()
));
test_bulk_type!(empty_nvarchar(
    "NVARCHAR(MAX)",
    100,
    vec![""; 100].into_iter()
));
test_bulk_type!(empty_varbinary(
    "VARBINARY(MAX)",
    100,
    vec![b""; 100].into_iter()
));

test_bulk_type!(real(
    "REAL",
    1000,
    vec![std::f32::consts::PI; 1000].into_iter()
));

test_bulk_type!(float(
    "FLOAT",
    1000,
    vec![std::f64::consts::PI; 1000].into_iter()
));

test_bulk_type!(varchar_limited(
    "VARCHAR(255)",
    1000,
    vec!["aaaaaaaaaaaaaaaaaaaaaaa"; 1000].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2(
    "DATETIME2",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_naive("DATETIME2", 100, {
    #[allow(deprecated)]
    let dt = NaiveDateTime::from_timestamp_opt(1658524194, 123456789).unwrap();

    vec![dt; 100].into_iter()
}));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_0(
    "DATETIME2(0)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_1(
    "DATETIME2(1)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_2(
    "DATETIME2(2)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_3(
    "DATETIME2(3)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_4(
    "DATETIME2(4)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_5(
    "DATETIME2(5)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_6(
    "DATETIME2(6)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));

#[cfg(all(feature = "tds73", feature = "chrono"))]
test_bulk_type!(datetime2_7(
    "DATETIME2(7)",
    100,
    vec![DateTime::from_timestamp(1658524194, 123456789); 100].into_iter()
));
