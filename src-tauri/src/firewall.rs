//! The Windows Firewall rule that lets the phone reach the PC (sync-design.md
//! §3.1): incoming connections to this program, on private networks only.
//!
//! It is added only when the user asks for it, after the Settings page has
//! explained why ("Allow Sync" the first time). Adding a firewall rule needs
//! administrator rights, so `netsh` runs elevated and Windows shows its own
//! permission prompt. Checking whether the rule exists needs no rights.
//!
//! The rule is bound to the program rather than to a port, so it also covers
//! mDNS (UDP 5353), with which the PC announces itself, and a changed port.

use std::path::Path;

use base64::Engine;

/// The rule's name, as shown in "Windows Defender Firewall with Advanced
/// Security" → Inbound Rules.
pub const RULE_NAME: &str = "Budgeting app (Sync)";

/// Windows' "the operation was cancelled by the user" (the permission
/// prompt was declined).
pub const CANCELLED: i32 = 1223;

/// The `netsh` arguments that add the rule for `program`.
pub fn add_rule_args(program: &Path) -> String {
    format!(
        "advfirewall firewall add rule name=\"{RULE_NAME}\" dir=in action=allow program=\"{}\" profile=private enable=yes",
        program.display()
    )
}

/// The PowerShell script that runs `netsh` elevated (Windows asks the user)
/// and exits with its exit code, or [`CANCELLED`] if the user said no.
pub fn elevated_script(program: &Path) -> String {
    let args = add_rule_args(program).replace('\'', "''");
    format!(
        "try {{ $p = Start-Process -FilePath 'netsh.exe' -ArgumentList '{args}' -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ErrorAction Stop; exit $p.ExitCode }} catch {{ exit {CANCELLED} }}"
    )
}

/// A script for `powershell -EncodedCommand`: base64 of its UTF-16LE text,
/// which avoids quoting the script on the command line.
pub fn encode_command(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Whether the rule is there for `program`, read from `netsh ... show rule`
/// output (the program path is printed as is in every Windows language).
pub fn rule_listed(netsh_output: &str, program: &Path) -> bool {
    let program = program.display().to_string().to_lowercase();
    netsh_output.to_lowercase().contains(&program)
}

#[cfg(windows)]
mod windows {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    /// No console window flashing up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    /// Whether the rule for `program` exists (no administrator rights needed).
    pub fn rule_exists(program: &Path) -> bool {
        let output = Command::new("netsh")
            .args(["advfirewall", "firewall", "show", "rule"])
            .arg(format!("name={}", super::RULE_NAME))
            .arg("verbose")
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        match output {
            Ok(out) if out.status.success() => {
                super::rule_listed(&String::from_utf8_lossy(&out.stdout), program)
            }
            _ => false,
        }
    }

    /// Add the rule. Windows asks the user for permission first.
    pub fn add_rule(program: &Path) -> Result<(), String> {
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
            .arg(super::encode_command(&super::elevated_script(program)))
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("could not start PowerShell: {e}"))?;
        match status.code() {
            Some(0) => Ok(()),
            Some(super::CANCELLED) => Err(
                "The firewall rule was not added, because Windows' permission prompt was declined."
                    .to_owned(),
            ),
            code => Err(format!(
                "Windows could not add the firewall rule (netsh exit code {code:?})."
            )),
        }
    }
}

/// Whether this PC needs the rule and has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallState {
    /// Not Windows: nothing to do.
    #[cfg_attr(windows, allow(dead_code))]
    NotNeeded,
    Allowed,
    Missing,
}

/// The state of the rule for the running program.
pub fn state() -> FirewallState {
    #[cfg(windows)]
    {
        match std::env::current_exe() {
            Ok(program) if windows::rule_exists(&program) => FirewallState::Allowed,
            _ => FirewallState::Missing,
        }
    }
    #[cfg(not(windows))]
    {
        FirewallState::NotNeeded
    }
}

/// Add the rule for the running program (Windows asks for permission).
pub fn allow() -> Result<(), String> {
    #[cfg(windows)]
    {
        let program = std::env::current_exe().map_err(|e| e.to_string())?;
        windows::add_rule(&program)
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_rule_allows_incoming_connections_to_this_program_on_private_networks_only() {
        let program = PathBuf::from(r"C:\Program Files\Budgeting\budgeting-app.exe");

        assert_eq!(
            add_rule_args(&program),
            r#"advfirewall firewall add rule name="Budgeting app (Sync)" dir=in action=allow program="C:\Program Files\Budgeting\budgeting-app.exe" profile=private enable=yes"#
        );
    }

    #[test]
    fn a_quote_in_the_program_path_does_not_break_the_elevated_script() {
        let program = PathBuf::from(r"C:\Users\O'Brien\budgeting-app.exe");

        let script = elevated_script(&program);

        assert!(
            script.contains(r#"program="C:\Users\O''Brien\budgeting-app.exe""#),
            "{script}"
        );
        assert!(script.contains("-Verb RunAs"), "{script}");
    }

    #[test]
    fn the_script_is_passed_to_powershell_as_base64_of_utf16() {
        assert_eq!(encode_command("exit 0"), "ZQB4AGkAdAAgADAA");
    }

    #[test]
    fn the_rule_counts_only_if_it_is_for_this_program() {
        let output =
            "Rule Name:   Budgeting app (Sync)\r\nProgram:   C:\\Apps\\Budgeting-App.exe\r\n";

        assert!(rule_listed(output, Path::new(r"C:\apps\budgeting-app.exe")));
        assert!(!rule_listed(
            output,
            Path::new(r"C:\dev\target\debug\budgeting-app.exe")
        ));
    }
}
