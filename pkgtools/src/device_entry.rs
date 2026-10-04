//! `DeviceEntry`: one device of EKA2L1's `devices.yml`, as the firmware recipe's stage.sh
//! takes it into the package's `device.yml` (symdev experiment 115 §1.4: a top-level key
//! per firmware code, its fields indented below it).

use crate::tool_error::{Result, ToolError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEntry {
    text: String,
}

impl DeviceEntry {
    /// The entry whose key is `firmcode`: its key line and every indented line after it.
    /// Its own `firmcode:` field must name the same code.
    pub fn find(devices_yml: &str, firmcode: &str) -> Result<DeviceEntry> {
        let key = format!("{firmcode}:");
        let mut lines = devices_yml.lines().skip_while(|l| *l != key);
        let Some(first) = lines.next() else {
            let there: Vec<&str> = devices_yml
                .lines()
                .filter(|l| !l.starts_with([' ', '\t']) && l.ends_with(':'))
                .map(|l| l.trim_end_matches(':'))
                .collect();
            return Err(ToolError::new(format!(
                "devices.yml has no device {firmcode}; it has: {}; install that firmware in \
                 EKA2L1 first",
                there.join(", ")
            )));
        };
        let mut text = format!("{first}\n");
        for line in lines.take_while(|l| l.starts_with([' ', '\t'])) {
            text.push_str(line);
            text.push('\n');
        }
        let code = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("firmcode:"))
            .map(str::trim);
        match code {
            Some(c) if c == firmcode => Ok(DeviceEntry { text }),
            Some(c) => Err(ToolError::new(format!(
                "devices.yml's entry {firmcode} has firmcode {c}"
            ))),
            None => Err(ToolError::new(format!(
                "devices.yml's entry {firmcode} has no firmcode field"
            ))),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceEntry;

    const TWO: &str = "RM-469:\n  platver: epoc93fp2\n  manufacturer: Nokia\n  firmcode: RM-469\n  model: N00\n  machine-uid: 0\n  isolated-drives: false\nRM-356:\n  platver: epoc94\n  firmcode: RM-356\n";

    #[test]
    fn takes_one_device_with_its_indented_lines() {
        let e = DeviceEntry::find(TWO, "RM-469").unwrap();
        assert_eq!(
            e.text(),
            "RM-469:\n  platver: epoc93fp2\n  manufacturer: Nokia\n  firmcode: RM-469\n  model: N00\n  machine-uid: 0\n  isolated-drives: false\n"
        );
    }

    #[test]
    fn the_last_device_ends_at_the_end_of_the_file() {
        let e = DeviceEntry::find(TWO, "RM-356").unwrap();
        assert_eq!(e.text(), "RM-356:\n  platver: epoc94\n  firmcode: RM-356\n");
    }

    #[test]
    fn a_missing_device_lists_the_ones_there() {
        let e = DeviceEntry::find(TWO, "RM-1").unwrap_err().to_string();
        assert!(e.contains("no device RM-1"), "{e}");
        assert!(e.contains("RM-469, RM-356"), "{e}");
    }

    #[test]
    fn an_entry_whose_firmcode_differs_is_refused() {
        let text = "RM-469:\n  firmcode: RM-470\n";
        let e = DeviceEntry::find(text, "RM-469").unwrap_err().to_string();
        assert!(e.contains("firmcode RM-470"), "{e}");
    }
}
