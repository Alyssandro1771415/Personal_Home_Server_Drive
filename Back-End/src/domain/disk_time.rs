use chrono::{DateTime, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

/// Data de modificação vem do mtime do disco. A criação usa a data de nascimento
/// do arquivo quando o sistema de arquivos guarda isso; senão, fica a mais antiga
/// entre o valor já salvo e o mtime. Assim um restart não regrava tudo com `now()`.
pub fn resolve_timestamps(
    existing_created: Option<DateTime<Utc>>,
    modified: SystemTime,
    birth: Option<SystemTime>,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let updated = to_utc(modified);
    let created = if let Some(birth) = birth {
        to_utc(birth)
    } else {
        match existing_created {
            Some(existing) if existing <= updated => existing,
            _ => updated,
        }
    };
    (created, updated)
}

fn to_utc(time: SystemTime) -> DateTime<Utc> {
    DateTime::<Utc>::from(time)
}

pub fn same_second(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    a.timestamp() == b.timestamp()
}

/// Evita pânico se o relógio do arquivo for anterior à época Unix.
pub fn system_time_or_epoch(time: Result<SystemTime, std::io::Error>) -> SystemTime {
    time.unwrap_or(UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn keeps_older_creation_and_uses_mtime_as_modification() {
        let existing = at(10);
        let mtime = UNIX_EPOCH + std::time::Duration::from_secs(50);
        let (created, updated) = resolve_timestamps(Some(existing), mtime, None);
        assert_eq!(created, existing);
        assert_eq!(updated.timestamp(), 50);
    }

    #[test]
    fn migration_stamp_newer_than_the_file_falls_back_to_mtime() {
        let stamped_at_boot = at(5_000);
        let mtime = UNIX_EPOCH + std::time::Duration::from_secs(100);
        let (created, updated) = resolve_timestamps(Some(stamped_at_boot), mtime, None);
        assert_eq!(created.timestamp(), 100);
        assert_eq!(updated.timestamp(), 100);
    }

    #[test]
    fn birth_time_wins_over_the_stored_stamp() {
        let stamped_at_boot = at(5_000);
        let mtime = UNIX_EPOCH + std::time::Duration::from_secs(100);
        let birth = UNIX_EPOCH + std::time::Duration::from_secs(20);
        let (created, updated) = resolve_timestamps(Some(stamped_at_boot), mtime, Some(birth));
        assert_eq!(created.timestamp(), 20);
        assert_eq!(updated.timestamp(), 100);
    }
}
