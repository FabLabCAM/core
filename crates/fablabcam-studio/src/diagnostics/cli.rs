use super::{LogLevel, Settings};
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Options {
    pub directory: Option<PathBuf>,
    pub level: Option<LogLevel>,
    pub full_backtrace: Option<bool>,
    pub core_on_panic: bool,
}

impl Options {
    /// Consume only our flags so their values cannot be mistaken for a project.
    pub fn parse(args: &mut Vec<String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut remaining = Vec::new();
        let mut input = args.iter();
        while let Some(arg) = input.next() {
            let mut value = || {
                input
                    .next()
                    .filter(|v| !v.starts_with("--"))
                    .ok_or_else(|| format!("Valeur manquante pour {arg}"))
            };
            match arg.as_str() {
                "--diagnostics" => options.level = Some(LogLevel::Debug),
                "--diagnostics-dir" => options.directory = Some(value()?.into()),
                "--log-level" => {
                    options.level = Some(match value()?.as_str() {
                        "normal" | "info" => LogLevel::Normal,
                        "debug" => LogLevel::Debug,
                        _ => return Err("--log-level : normal ou debug".into()),
                    })
                }
                "--backtrace" => {
                    options.full_backtrace = Some(match value()?.as_str() {
                        "full" => true,
                        "off" => false,
                        _ => return Err("--backtrace : off ou full".into()),
                    })
                }
                "--core-on-panic" => options.core_on_panic = true,
                _ => remaining.push(arg.clone()),
            }
        }
        *args = remaining;
        Ok(options)
    }

    pub fn settings(&self, mut defaults: Settings) -> Settings {
        if let Some(level) = self.level {
            defaults.level = level;
        }
        if let Some(full) = self.full_backtrace {
            defaults.full_backtrace = full;
        }
        defaults
    }
}
