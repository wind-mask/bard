use std::ffi::{OsStr, OsString};

use anyhow::{Result, bail};

pub const DEFAULT_OFFSET_MS: i64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cli {
    pub offset_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliAction {
    Run(Cli),
    Help,
    Version,
}

pub fn parse() -> Result<CliAction> {
    parse_from(std::env::args_os().skip(1))
}

fn parse_from<I>(args: I) -> Result<CliAction>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let mut offset_ms = DEFAULT_OFFSET_MS;

    while let Some(argument) = args.next() {
        if argument == OsStr::new("--help") || argument == OsStr::new("-h") {
            return Ok(CliAction::Help);
        }
        if argument == OsStr::new("--version") || argument == OsStr::new("-V") {
            return Ok(CliAction::Version);
        }
        if argument == OsStr::new("--offset-ms") {
            let Some(value) = args.next() else {
                bail!("--offset-ms requires a signed integer value");
            };
            offset_ms = parse_offset(&value)?;
            continue;
        }
        if let Some(value) = argument
            .to_str()
            .and_then(|value| value.strip_prefix("--offset-ms="))
        {
            offset_ms = value
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid --offset-ms value: {value}"))?;
            continue;
        }

        bail!("unknown argument: {}", argument.to_string_lossy());
    }

    Ok(CliAction::Run(Cli { offset_ms }))
}

fn parse_offset(value: &OsStr) -> Result<i64> {
    let value = value
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("--offset-ms must be valid UTF-8"))?;
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid --offset-ms value: {value}"))
}

pub fn help() -> &'static str {
    "waybar-bard - display synchronized local lyrics\n\nUsage: waybar-bard [OPTIONS]\n\nOptions:\n  --offset-ms <MILLISECONDS>  Global lyric calibration offset [default: 100]\n  -h, --help                   Print help\n  -V, --version                Print version\n"
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{Cli, CliAction, DEFAULT_OFFSET_MS, parse_from};

    fn args<'a>(values: &'a [&'a str]) -> impl Iterator<Item = OsString> + 'a {
        values.iter().map(OsString::from)
    }

    #[test]
    fn uses_default_and_parses_signed_offsets() {
        assert_eq!(
            parse_from(args(&[])).unwrap(),
            CliAction::Run(Cli {
                offset_ms: DEFAULT_OFFSET_MS
            })
        );
        assert_eq!(
            parse_from(args(&["--offset-ms", "-250"])).unwrap(),
            CliAction::Run(Cli { offset_ms: -250 })
        );
        assert_eq!(
            parse_from(args(&["--offset-ms=300"])).unwrap(),
            CliAction::Run(Cli { offset_ms: 300 })
        );
    }

    #[test]
    fn handles_help_version_and_invalid_arguments() {
        assert_eq!(parse_from(args(&["--help"])).unwrap(), CliAction::Help);
        assert_eq!(parse_from(args(&["-V"])).unwrap(), CliAction::Version);
        assert!(parse_from(args(&["--offset-ms"])).is_err());
        assert!(parse_from(args(&["--offset-ms", "bad"])).is_err());
        assert!(parse_from(args(&["--unknown"])).is_err());
    }
}
