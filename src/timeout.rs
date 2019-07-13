use chrono::naive::NaiveTime;
use std::str::FromStr;
use std::string::ToString;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Timeout {
    timeout: NaiveTime,
}

impl FromStr for Timeout {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let timeout = NaiveTime::parse_from_str(value, "%H:%M:%S").map_err(|err| {
            format!(
                "Failed parsing Timeout from '{}' with error: {}",
                value, err
            )
        })?;
        if timeout == NaiveTime::from_hms(0, 0, 0) {
            Err("Timeout of '00:00:00' is not allowed".to_owned())
        } else {
            Ok(Self { timeout })
        }
    }
}

impl ToString for Timeout {
    fn to_string(&self) -> String {
        self.timeout.format("%H:%M:%S").to_string()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn valid_input() {
        assert_eq!(
            Timeout::from_str("00:00:10"),
            Ok(Timeout {
                timeout: NaiveTime::from_hms(0, 0, 10)
            })
        );
        assert_eq!(
            Timeout::from_str("00:10:00"),
            Ok(Timeout {
                timeout: NaiveTime::from_hms(0, 10, 0)
            })
        );
        assert_eq!(
            Timeout::from_str("10:00:00"),
            Ok(Timeout {
                timeout: NaiveTime::from_hms(10, 0, 0)
            })
        );
        assert_eq!(
            Timeout::from_str("23:59:59"),
            Ok(Timeout {
                timeout: NaiveTime::from_hms(23, 59, 59)
            })
        );
    }

    #[test]
    fn invalid_input() {
        assert_eq!(
            Timeout::from_str("10"),
            Err("Failed parsing Timeout from '10' with error: premature end of input".to_owned())
        );
        assert_eq!(
            Timeout::from_str("10:00"),
            Err(
                "Failed parsing Timeout from '10:00' with error: premature end of input".to_owned()
            )
        );
        assert_eq!(
            Timeout::from_str(""),
            Err("Failed parsing Timeout from '' with error: premature end of input".to_owned())
        );
        assert_eq!(
            Timeout::from_str("24:00:00"),
            Err(
                "Failed parsing Timeout from '24:00:00' with error: input is out of range"
                    .to_owned()
            )
        );
        assert_eq!(
            Timeout::from_str("00:00:00"),
            Err("Timeout of '00:00:00' is not allowed".to_owned())
        );
    }
}
