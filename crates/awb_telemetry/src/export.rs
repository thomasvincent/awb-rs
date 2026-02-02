use crate::events::TelemetryEvent;
use std::io::Write;

pub enum ExportFormat { Csv, PlainText, Json }

pub fn export_log(events: &[TelemetryEvent], format: ExportFormat, writer: &mut dyn Write) -> Result<(), std::io::Error> {
    match format {
        ExportFormat::Json => {
            for event in events {
                serde_json::to_writer(&mut *writer, event)?;
                writeln!(writer)?;
            }
        }
        ExportFormat::PlainText => {
            for event in events {
                writeln!(writer, "{:?}", event)?;
            }
        }
        ExportFormat::Csv => {
            writeln!(writer, "type,timestamp,details")?;
            for event in events {
                let json = serde_json::to_string(event).unwrap_or_default();
                writeln!(writer, "{}", json)?;
            }
        }
    }
    Ok(())
}
