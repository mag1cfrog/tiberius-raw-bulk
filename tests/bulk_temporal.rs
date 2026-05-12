use futures_util::io::{AsyncRead, AsyncWrite};
use names::{Generator, Name};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::env;
use std::sync::Once;
use tiberius::time::{Date, DateTime2, DateTimeOffset, Time};
use tiberius::{ColumnData, Result, TokenRow};

use runtimes_macro::test_on_runtimes;

#[allow(dead_code)]
static LOGGER_SETUP: Once = Once::new();

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("TIBERIUS_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;user=SA;password=<YourStrong@Passw0rd>;TrustServerCertificate=true"
            .to_owned()
    })
});

thread_local! {
    static NAMES: RefCell<Option<Generator<'static>>> = RefCell::new(None);
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

async fn create_temporal_table<S>(
    conn: &mut tiberius::Client<S>,
    table: &str,
    sql_type: &str,
    nullable: &str,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    conn.execute(
        &format!(
            "CREATE TABLE {table} (id INT IDENTITY PRIMARY KEY, content {sql_type} {nullable})",
        ),
        &[],
    )
    .await?;

    Ok(())
}

async fn bulk_insert_temporal_rows<S>(
    conn: &mut tiberius::Client<S>,
    table: &str,
    values: impl IntoIterator<Item = ColumnData<'static>>,
) -> Result<u64>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let mut req = conn.bulk_insert(table).await?;

    for value in values {
        let mut row = TokenRow::new();
        row.push(value);
        req.send(row).await?;
    }

    Ok(req.finalize().await?.total())
}

async fn select_temporal_strings<S>(
    conn: &mut tiberius::Client<S>,
    table: &str,
) -> Result<Vec<Option<String>>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let rows = conn
        .query(
            format!("SELECT CONVERT(VARCHAR(50), content, 126) FROM {table} ORDER BY id",),
            &[],
        )
        .await?
        .into_first_result()
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| row.get::<&str, _>(0).map(ToOwned::to_owned))
        .collect())
}

async fn temporal_round_trip<S>(
    mut conn: tiberius::Client<S>,
    sql_type: &str,
    nullable: &str,
    values: Vec<ColumnData<'static>>,
    expected: &[Option<&str>],
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);
    create_temporal_table(&mut conn, &table, sql_type, nullable).await?;

    let total = bulk_insert_temporal_rows(&mut conn, &table, values).await?;

    assert_eq!(expected.len() as u64, total);
    assert_eq!(
        expected
            .iter()
            .map(|value| value.map(ToOwned::to_owned))
            .collect::<Vec<_>>(),
        select_temporal_strings(&mut conn, &table).await?,
    );

    Ok(())
}

async fn raw_date_payload_round_trip<S>(
    mut conn: tiberius::Client<S>,
    payload: &'static [u8],
) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    let table = format!("##{}", random_table().await);
    create_temporal_table(&mut conn, &table, "DATE", "NOT NULL").await?;

    let mut req = conn.bulk_insert(&table).await?;
    req.send_raw_row_payload(payload).await?;
    let total = req.finalize().await?.total();

    assert_eq!(1, total);

    Ok(())
}

#[test_on_runtimes]
async fn bulk_load_raw_date_payload_with_length<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    raw_date_payload_round_trip(conn, &[3, 0, 0, 0]).await
}

#[test_on_runtimes]
async fn bulk_load_required_date_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "DATE",
        "NOT NULL",
        vec![
            ColumnData::Date(Some(Date::new(0))),
            ColumnData::Date(Some(Date::new(1))),
        ],
        &[Some("0001-01-01"), Some("0001-01-02")],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_optional_date_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "DATE",
        "NULL",
        vec![ColumnData::Date(None), ColumnData::Date(Some(Date::new(0)))],
        &[None, Some("0001-01-01")],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_required_time_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "TIME(3)",
        "NOT NULL",
        vec![
            ColumnData::Time(Some(Time::new(0, 3))),
            ColumnData::Time(Some(Time::new(45_296_123, 3))),
        ],
        &[Some("00:00:00"), Some("12:34:56.123")],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_optional_time_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "TIME(3)",
        "NULL",
        vec![
            ColumnData::Time(None),
            ColumnData::Time(Some(Time::new(45_296_123, 3))),
        ],
        &[None, Some("12:34:56.123")],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_required_datetime2_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "DATETIME2(3)",
        "NOT NULL",
        vec![
            ColumnData::DateTime2(Some(DateTime2::new(Date::new(0), Time::new(0, 3)))),
            ColumnData::DateTime2(Some(DateTime2::new(Date::new(1), Time::new(45_296_123, 3)))),
        ],
        &[Some("0001-01-01T00:00:00"), Some("0001-01-02T12:34:56.123")],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_optional_datetime2_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "DATETIME2(3)",
        "NULL",
        vec![
            ColumnData::DateTime2(None),
            ColumnData::DateTime2(Some(DateTime2::new(Date::new(1), Time::new(45_296_123, 3)))),
        ],
        &[None, Some("0001-01-02T12:34:56.123")],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_required_datetimeoffset_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "DATETIMEOFFSET(3)",
        "NOT NULL",
        vec![
            ColumnData::DateTimeOffset(Some(DateTimeOffset::new(
                DateTime2::new(Date::new(0), Time::new(0, 3)),
                0,
            ))),
            ColumnData::DateTimeOffset(Some(DateTimeOffset::new(
                DateTime2::new(Date::new(1), Time::new(70_496_123, 3)),
                -420,
            ))),
        ],
        &[
            Some("0001-01-01T00:00:00+00:00"),
            Some("0001-01-02T12:34:56.123-07:00"),
        ],
    )
    .await
}

#[test_on_runtimes]
async fn bulk_load_optional_datetimeoffset_column_data<S>(conn: tiberius::Client<S>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    temporal_round_trip(
        conn,
        "DATETIMEOFFSET(3)",
        "NULL",
        vec![
            ColumnData::DateTimeOffset(None),
            ColumnData::DateTimeOffset(Some(DateTimeOffset::new(
                DateTime2::new(Date::new(1), Time::new(25_496_123, 3)),
                330,
            ))),
        ],
        &[None, Some("0001-01-02T12:34:56.123+05:30")],
    )
    .await
}
