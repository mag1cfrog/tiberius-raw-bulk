use super::{event, target};
use crate::tds::codec::{
    DoneStatus, EnvChangeTy, FeatureAck, FeatureLevel, FedAuthAck, TokenDone, TokenEnvChange,
    TokenError, TokenFeatureExtAck, TokenInfo, TokenLoginAck,
};
use tracing::Level;

/// DONE token family.
#[derive(Clone, Copy, Debug)]
pub(crate) enum DoneKind {
    Done,
    DoneProc,
    DoneInProc,
}

impl DoneKind {
    fn name(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::DoneProc => "done_proc",
            Self::DoneInProc => "done_in_proc",
        }
    }
}

/// Emits safe column metadata token telemetry.
pub(crate) fn emit_col_metadata(column_count: usize) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_COL_METADATA,
        phase = "token",
        token_kind = "col_metadata",
        column_count = count(column_count),
    );
}

/// Emits safe row token telemetry without row values.
pub(crate) fn emit_row(column_count: usize) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_ROW,
        phase = "token",
        token_kind = "row",
        column_count = count(column_count),
    );
}

/// Emits safe NBC row token telemetry without row values or null bitmap bytes.
pub(crate) fn emit_nbc_row(column_count: usize) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_NBC_ROW,
        phase = "token",
        token_kind = "nbc_row",
        column_count = count(column_count),
    );
}

/// Emits safe return value token telemetry without parameter names or values.
pub(crate) fn emit_return_value(udf: bool) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_RETURN_VALUE,
        phase = "token",
        token_kind = "return_value",
        udf = udf,
    );
}

/// Emits safe return status token telemetry without the status value.
pub(crate) fn emit_return_status() {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_RETURN_STATUS,
        phase = "token",
        token_kind = "return_status",
    );
}

/// Emits safe order token telemetry without raw column order values.
pub(crate) fn emit_order(column_count: usize) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_ORDER,
        phase = "token",
        token_kind = "order",
        column_count = count(column_count),
    );
}

/// Emits safe DONE-family token telemetry.
pub(crate) fn emit_done(kind: DoneKind, done: &TokenDone) {
    match done.row_count() {
        Some(row_count) => tracing::event!(
            target: target::PROTOCOL,
            Level::TRACE,
            telemetry_event = event::TOKEN_DONE,
            phase = "token",
            token_kind = kind.name(),
            done_final = done.is_final(),
            done_more = done.status_contains(DoneStatus::More),
            done_error = done.status_contains(DoneStatus::Error),
            done_inexact = done.status_contains(DoneStatus::Inexact),
            done_count_valid = true,
            done_attention = done.status_contains(DoneStatus::Attention),
            done_rpc_in_batch = done.status_contains(DoneStatus::RpcInBatch),
            done_srv_error = done.status_contains(DoneStatus::SrvError),
            row_count = row_count,
        ),
        None => tracing::event!(
            target: target::PROTOCOL,
            Level::TRACE,
            telemetry_event = event::TOKEN_DONE,
            phase = "token",
            token_kind = kind.name(),
            done_final = done.is_final(),
            done_more = done.status_contains(DoneStatus::More),
            done_error = done.status_contains(DoneStatus::Error),
            done_inexact = done.status_contains(DoneStatus::Inexact),
            done_count_valid = false,
            done_attention = done.status_contains(DoneStatus::Attention),
            done_rpc_in_batch = done.status_contains(DoneStatus::RpcInBatch),
            done_srv_error = done.status_contains(DoneStatus::SrvError),
        ),
    }
}

/// Emits safe server error token telemetry without message text or names.
pub(crate) fn emit_error(error: &TokenError) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::WARN,
        telemetry_event = event::TOKEN_ERROR,
        phase = "token",
        token_kind = "error",
        error_code = error.code(),
        error_state = error.state(),
        error_class = error.class(),
        error_line = error.line(),
    );
}

/// Emits safe server info token telemetry without message text or names.
pub(crate) fn emit_info(info: &TokenInfo) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TOKEN_INFO,
        phase = "token",
        token_kind = "info",
        info_number = info.number,
        info_state = info.state,
        info_class = info.class,
        info_line = info.line,
    );
}

/// Emits safe environment change token telemetry without string values.
pub(crate) fn emit_env_change(change: &TokenEnvChange) {
    match change {
        TokenEnvChange::PacketSize(new_size, old_size) => tracing::event!(
            target: target::PROTOCOL,
            Level::INFO,
            telemetry_event = event::TOKEN_ENV_CHANGE,
            phase = "token",
            token_kind = "env_change",
            env_change_kind = env_change_kind(change),
            old_packet_size_bytes = u64::from(*old_size),
            new_packet_size_bytes = u64::from(*new_size),
        ),
        TokenEnvChange::SqlCollation { old, new } => tracing::event!(
            target: target::PROTOCOL,
            Level::INFO,
            telemetry_event = event::TOKEN_ENV_CHANGE,
            phase = "token",
            token_kind = "env_change",
            env_change_kind = env_change_kind(change),
            old_collation_present = old.is_some(),
            new_collation_present = new.is_some(),
        ),
        _ => tracing::event!(
            target: target::PROTOCOL,
            Level::INFO,
            telemetry_event = event::TOKEN_ENV_CHANGE,
            phase = "token",
            token_kind = "env_change",
            env_change_kind = env_change_kind(change),
        ),
    }
}

/// Emits safe login acknowledgement telemetry without program names.
pub(crate) fn emit_login_ack(ack: &TokenLoginAck) {
    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TOKEN_LOGIN_ACK,
        phase = "token",
        token_kind = "login_ack",
        interface = u64::from(ack.interface),
        tds_version = feature_level_name(ack.tds_version),
        server_version = u64::from(ack.version),
    );
}

/// Emits safe feature extension acknowledgement telemetry without nonce bytes.
pub(crate) fn emit_feature_ext_ack(ack: &TokenFeatureExtAck) {
    let (fed_auth_count, fed_auth_nonce_count) = feature_ext_summary(ack);

    tracing::event!(
        target: target::PROTOCOL,
        Level::INFO,
        telemetry_event = event::TOKEN_FEATURE_EXT_ACK,
        phase = "token",
        token_kind = "feature_ext_ack",
        feature_count = count(ack.features.len()),
        fed_auth_count = fed_auth_count,
        fed_auth_nonce_count = fed_auth_nonce_count,
    );
}

/// Emits safe SSPI token telemetry without auth payload bytes.
pub(crate) fn emit_sspi() {
    tracing::event!(
        target: target::PROTOCOL,
        Level::TRACE,
        telemetry_event = event::TOKEN_SSPI,
        phase = "token",
        token_kind = "sspi",
    );
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn env_change_kind(change: &TokenEnvChange) -> &'static str {
    match change {
        TokenEnvChange::Database(_, _) => "database",
        TokenEnvChange::PacketSize(_, _) => "packet_size",
        TokenEnvChange::SqlCollation { .. } => "sql_collation",
        TokenEnvChange::BeginTransaction(_) => "begin_transaction",
        TokenEnvChange::CommitTransaction => "commit_transaction",
        TokenEnvChange::RollbackTransaction => "rollback_transaction",
        TokenEnvChange::DefectTransaction => "defect_transaction",
        TokenEnvChange::Routing { .. } => "routing",
        TokenEnvChange::ChangeMirror(_) => "change_mirror",
        TokenEnvChange::Ignored(ty) => ignored_env_change_kind(*ty),
    }
}

fn ignored_env_change_kind(ty: EnvChangeTy) -> &'static str {
    match ty {
        EnvChangeTy::Database => "database",
        EnvChangeTy::Language => "language",
        EnvChangeTy::CharacterSet => "character_set",
        EnvChangeTy::PacketSize => "packet_size",
        EnvChangeTy::UnicodeDataSortingLID => "unicode_data_sorting_lid",
        EnvChangeTy::UnicodeDataSortingCFL => "unicode_data_sorting_cfl",
        EnvChangeTy::SqlCollation => "sql_collation",
        EnvChangeTy::BeginTransaction => "begin_transaction",
        EnvChangeTy::CommitTransaction => "commit_transaction",
        EnvChangeTy::RollbackTransaction => "rollback_transaction",
        EnvChangeTy::EnlistDTCTransaction => "enlist_dtc_transaction",
        EnvChangeTy::DefectTransaction => "defect_transaction",
        EnvChangeTy::Rtls => "rtls",
        EnvChangeTy::PromoteTransaction => "promote_transaction",
        EnvChangeTy::TransactionManagerAddress => "transaction_manager_address",
        EnvChangeTy::TransactionEnded => "transaction_ended",
        EnvChangeTy::ResetConnection => "reset_connection",
        EnvChangeTy::UserName => "user_name",
        EnvChangeTy::Routing => "routing",
    }
}

fn feature_level_name(feature_level: FeatureLevel) -> &'static str {
    match feature_level {
        FeatureLevel::SqlServerV7 => "sql_server_v7",
        FeatureLevel::SqlServer2000 => "sql_server_2000",
        FeatureLevel::SqlServer2000Sp1 => "sql_server_2000_sp1",
        FeatureLevel::SqlServer2005 => "sql_server_2005",
        FeatureLevel::SqlServer2008 => "sql_server_2008",
        FeatureLevel::SqlServer2008R2 => "sql_server_2008_r2",
        FeatureLevel::SqlServerN => "sql_server_n",
    }
}

fn feature_ext_summary(ack: &TokenFeatureExtAck) -> (u64, u64) {
    let mut fed_auth_count = 0_u64;
    let mut fed_auth_nonce_count = 0_u64;

    for feature in &ack.features {
        match feature {
            FeatureAck::FedAuth(FedAuthAck::SecurityToken { nonce }) => {
                fed_auth_count = fed_auth_count.saturating_add(1);
                if nonce.is_some() {
                    fed_auth_nonce_count = fed_auth_nonce_count.saturating_add(1);
                }
            }
        }
    }

    (fed_auth_count, fed_auth_nonce_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        observability::{event, field, target, test_support},
        tds::codec::{TokenDone, TokenEnvChange, TokenError, TokenFeatureExtAck, TokenInfo},
    };
    use tracing::Level;

    #[test]
    fn done_helpers_emit_safe_status_and_count_metadata() {
        let done = TokenDone::for_test(
            DoneStatus::More | DoneStatus::Count | DoneStatus::RpcInBatch,
            42,
        );

        let (_output, records) = test_support::capture(|| {
            emit_done(DoneKind::DoneProc, &done);
        });

        let event = records
            .event(event::TOKEN_DONE)
            .unwrap_or_else(|| panic!("missing done event in {records:?}"));
        assert_eq!(target::PROTOCOL, event.target);
        assert_eq!(Level::TRACE, event.level);
        event.assert_field(field::TELEMETRY_EVENT, event::TOKEN_DONE);
        event.assert_field("token_kind", "done_proc");
        event.assert_field("done_final", "false");
        event.assert_field("done_more", "true");
        event.assert_field("done_count_valid", "true");
        event.assert_field("done_rpc_in_batch", "true");
        event.assert_field("row_count", "42");
    }

    #[test]
    fn done_helpers_omit_unknown_row_count() {
        let done = TokenDone::for_test(DoneStatus::More.into(), 42);

        let (_output, records) = test_support::capture(|| {
            emit_done(DoneKind::Done, &done);
        });

        let event = records
            .event(event::TOKEN_DONE)
            .unwrap_or_else(|| panic!("missing done event in {records:?}"));
        event.assert_field("done_count_valid", "false");
        assert_eq!(None, event.field("row_count"));
    }

    #[test]
    fn error_and_info_helpers_emit_metadata_without_server_text() {
        let error = TokenError {
            code: 208,
            state: 1,
            class: 16,
            message: "Invalid object name 'secret_table'".to_string(),
            server: "secret-server".to_string(),
            procedure: "secret_proc".to_string(),
            line: 7,
        };
        let info = TokenInfo {
            number: 5701,
            state: 2,
            class: 0,
            message: "Changed database context to 'secret_db'".to_string(),
            server: "secret-server".to_string(),
            procedure: "secret_proc".to_string(),
            line: 9,
        };

        let (_output, records) = test_support::capture(|| {
            emit_error(&error);
            emit_info(&info);
        });

        records.assert_no_forbidden_text(&[
            "Invalid object name",
            "secret_table",
            "Changed database context",
            "secret_db",
            "secret-server",
            "secret_proc",
        ]);

        let error_event = records
            .event(event::TOKEN_ERROR)
            .unwrap_or_else(|| panic!("missing error event in {records:?}"));
        assert_eq!(Level::WARN, error_event.level);
        error_event.assert_field("error_code", "208");
        error_event.assert_field("error_class", "16");
        error_event.assert_field("error_line", "7");

        let info_event = records
            .event(event::TOKEN_INFO)
            .unwrap_or_else(|| panic!("missing info event in {records:?}"));
        assert_eq!(Level::INFO, info_event.level);
        info_event.assert_field("info_number", "5701");
        info_event.assert_field("info_class", "0");
        info_event.assert_field("info_line", "9");
    }

    #[test]
    fn env_change_helper_omits_unsafe_string_values() {
        let changes = [
            TokenEnvChange::Database("secret_new_db".to_string(), "secret_old_db".to_string()),
            TokenEnvChange::Routing {
                host: "secret-routing-host.example.com".to_string(),
                port: 1433,
            },
            TokenEnvChange::ChangeMirror("secret-mirror-host".to_string()),
        ];

        let (_output, records) = test_support::capture(|| {
            for change in &changes {
                emit_env_change(change);
            }
        });

        records.assert_no_forbidden_text(&[
            "secret_new_db",
            "secret_old_db",
            "secret-routing-host",
            "secret-mirror-host",
        ]);

        let env_change = records
            .event(event::TOKEN_ENV_CHANGE)
            .unwrap_or_else(|| panic!("missing env change event in {records:?}"));
        env_change.assert_field("token_kind", "env_change");
        env_change.assert_field("env_change_kind", "database");
    }

    #[test]
    fn env_change_helper_emits_packet_size_metadata() {
        let (_output, records) = test_support::capture(|| {
            emit_env_change(&TokenEnvChange::PacketSize(8192, 4096));
        });

        let packet_size = records
            .event(event::TOKEN_ENV_CHANGE)
            .unwrap_or_else(|| panic!("missing env change event in {records:?}"));
        packet_size.assert_field("env_change_kind", "packet_size");
        packet_size.assert_field("old_packet_size_bytes", "4096");
        packet_size.assert_field("new_packet_size_bytes", "8192");
    }

    #[test]
    fn login_and_feature_ack_helpers_emit_safe_metadata() {
        let login_ack = TokenLoginAck {
            interface: 1,
            tds_version: FeatureLevel::SqlServerN,
            prog_name: "secret-sql-server-build-name".to_string(),
            version: 0x1000_0001,
        };
        let feature_ack = TokenFeatureExtAck {
            features: vec![FeatureAck::FedAuth(FedAuthAck::SecurityToken {
                nonce: Some([7; 32]),
            })],
        };

        let (_output, records) = test_support::capture(|| {
            emit_login_ack(&login_ack);
            emit_feature_ext_ack(&feature_ack);
        });

        records.assert_no_forbidden_text(&["secret-sql-server-build-name", "[7", "7, 7"]);

        let login = records
            .event(event::TOKEN_LOGIN_ACK)
            .unwrap_or_else(|| panic!("missing login ack event in {records:?}"));
        login.assert_field("interface", "1");
        login.assert_field("tds_version", "sql_server_n");
        login.assert_field("server_version", "268435457");

        let feature = records
            .event(event::TOKEN_FEATURE_EXT_ACK)
            .unwrap_or_else(|| panic!("missing feature ext ack event in {records:?}"));
        feature.assert_field("feature_count", "1");
        feature.assert_field("fed_auth_count", "1");
        feature.assert_field("fed_auth_nonce_count", "1");
    }

    #[test]
    fn row_return_and_sspi_helpers_do_not_emit_payload_values() {
        let (_output, records) = test_support::capture(|| {
            emit_row(3);
            emit_nbc_row(3);
            emit_return_value(true);
            emit_sspi();
        });

        records.assert_no_forbidden_text(&[
            "secret row value",
            "secret_output_param",
            "SSPI_PAYLOAD_BYTES",
            "GSSAPI_PAYLOAD_BYTES",
        ]);

        let row = records
            .event(event::TOKEN_ROW)
            .unwrap_or_else(|| panic!("missing row event in {records:?}"));
        row.assert_field("column_count", "3");

        let return_value = records
            .event(event::TOKEN_RETURN_VALUE)
            .unwrap_or_else(|| panic!("missing return value event in {records:?}"));
        return_value.assert_field("udf", "true");

        let sspi = records
            .event(event::TOKEN_SSPI)
            .unwrap_or_else(|| panic!("missing sspi event in {records:?}"));
        sspi.assert_field("token_kind", "sspi");
    }

    #[test]
    fn helpers_succeed_without_subscriber() {
        test_support::with_no_subscriber(|| {
            emit_return_status();
            emit_order(2);
            emit_sspi();
        });
    }
}
