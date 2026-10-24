//! AWS sandboxes through the user's own AWS CLI login (profiles, SSO or `aws login`):
//! EC2 desktops for Linux (Ubuntu, arm64) and Windows Server, reached only through
//! Systems Manager. No SSH key pair or VNC ingress rule is added; the account's default
//! VPC security-group policy still applies. Commands run with SSM Run Command and the
//! screen is VNC bound to the instance's localhost, forwarded by an SSM port-forwarding
//! session. Instances are tagged `pandacode=sandbox`.
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, process::Command, time::Duration};

use super::Machine;
use crate::workspace::run_process;

pub const ROLE: &str = "PandaCodeSandboxRole";
pub const PROFILE: &str = "PandaCodeSandboxProfile";
const QUICK: Duration = Duration::from_secs(90);

/// This file contains guest passwords. Create it exclusively with private Unix
/// permissions before writing any bytes, and clean up on both success and error.
struct UserDataFile {
    path: PathBuf,
}
impl UserDataFile {
    fn create(contents: &str) -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("pandacode-user-data-{}.txt", uuid::Uuid::new_v4()));
        Self::create_at(path, contents).map_err(|e| format!("Could not write private AWS user data: {e}"))
    }
    fn create_at(path: PathBuf, contents: &str) -> std::io::Result<Self> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        // Construct the guard only after create_new succeeds: a pre-existing
        // file or symlink must neither be changed nor removed on refusal.
        let temporary = Self { path };
        let written = file.write_all(contents.as_bytes()).and_then(|_| file.sync_all());
        drop(file);
        written?;
        Ok(temporary)
    }
}
impl Drop for UserDataFile {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.path); }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Aws {
    pub profile: Option<String>,
    pub region: String,
}

/// What each desktop OS needs.
pub struct Flavor {
    pub parameter: &'static str,
    pub instance_type: &'static str,
    pub disk_gb: u32,
    pub vnc_port: u16,
}
pub const LINUX: Flavor = Flavor { parameter: "/aws/service/canonical/ubuntu/server/24.04/stable/current/arm64/hvm/ebs-gp3/ami-id", instance_type: "t4g.medium", disk_gb: 20, vnc_port: 5901 };
pub const WINDOWS: Flavor = Flavor { parameter: "/aws/service/ami-windows-latest/Windows_Server-2025-English-Full-Base", instance_type: "t3.large", disk_gb: 50, vnc_port: 5900 };
pub fn flavor(os: &str) -> Result<&'static Flavor, String> {
    match os { "linux" => Ok(&LINUX), "windows" => Ok(&WINDOWS), _ => Err("AWS desktops run linux or windows.".into()) }
}

/// The first line of an AWS CLI error, without the "An error occurred" boilerplate.
pub fn cli_error(stderr: &str) -> String {
    let line = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("The AWS CLI failed.");
    let line = line.trim_start_matches("aws: [ERROR]: ");
    if line.contains("Unable to locate credentials") || line.contains("Token has expired") || line.contains("ExpiredToken") || line.contains("SSO session") {
        return "AWS credentials are missing or expired. Run `aws login` (or `aws sso login`) in a terminal, then try again.".into();
    }
    match line.split_once(" operation: ").or_else(|| line.split_once("): ")) {
        Some((_, message)) => format!("AWS: {message}"),
        None => format!("AWS: {line}"),
    }
}

/// Linux user data: a light Xfce desktop on VNC display :1, bound to localhost.
pub fn linux_user_data(password: &str) -> String {
    format!(r#"#!/bin/bash
set +x
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -q
apt-get install -y -q --no-install-recommends xfce4 xfce4-terminal dbus-x11 tigervnc-standalone-server tigervnc-common tigervnc-tools fonts-dejavu-core xdg-utils
umask 077
install -d -o ubuntu -g ubuntu -m 700 /home/ubuntu/.vnc
printf '%s\n' {password} | vncpasswd -f > /home/ubuntu/.vnc/passwd
chown ubuntu:ubuntu /home/ubuntu/.vnc/passwd && chmod 600 /home/ubuntu/.vnc/passwd
printf '#!/bin/sh\nunset SESSION_MANAGER DBUS_SESSION_BUS_ADDRESS\nxset s off -dpms\nexec startxfce4\n' > /home/ubuntu/.vnc/xstartup
chown ubuntu:ubuntu /home/ubuntu/.vnc/xstartup && chmod 755 /home/ubuntu/.vnc/xstartup
cat > /etc/systemd/system/pandacode-vnc.service <<'UNIT'
[Unit]
Description=PandaCode desktop (VNC on localhost:5901)
After=network.target
[Service]
Type=forking
User=ubuntu
ExecStartPre=-/usr/bin/vncserver -kill :1
ExecStart=/usr/bin/vncserver :1 -localhost yes -geometry 1280x800 -depth 24 -SecurityTypes VncAuth
ExecStop=/usr/bin/vncserver -kill :1
Restart=on-failure
[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable --now pandacode-vnc.service
"#, password = super::shell_quote(password))
}

/// Windows user data: TightVNC on loopback only, and automatic sign-in so the screen shows
/// a desktop rather than the lock screen. EC2 user data and the guest's Winlogon
/// configuration necessarily contain this guest password; they are not a secret vault.
pub fn windows_user_data(password: &str) -> String {
    let vnc = &password[..8.min(password.len())];
    format!(r#"<powershell>
$ErrorActionPreference = 'Stop'
Set-PSDebug -Off
$staging = Join-Path $env:TEMP ('PandaCode-TightVNC-' + [guid]::NewGuid().ToString('N'))
if (Test-Path -LiteralPath $staging) {{ throw 'Installer staging already exists.' }}
$acl = [System.Security.AccessControl.DirectorySecurity]::new()
$acl.SetAccessRuleProtection($true, $false)
foreach ($sid in @('S-1-5-18', 'S-1-5-32-544')) {{
    $identity = [System.Security.Principal.SecurityIdentifier]::new($sid)
    $rule = [System.Security.AccessControl.FileSystemAccessRule]::new($identity, 'FullControl', 'ContainerInherit, ObjectInherit', 'None', 'Allow')
    $acl.AddAccessRule($rule)
}}
[System.IO.Directory]::CreateDirectory($staging, $acl) | Out-Null
$msi = Join-Path $staging 'tightvnc.msi'
try {{
    Invoke-WebRequest -UseBasicParsing -MaximumRedirection 0 -TimeoutSec 120 -Uri 'https://www.tightvnc.com/download/2.8.90/tightvnc-2.8.90-gpl-setup-64bit.msi' -OutFile $msi
    $signature = Get-AuthenticodeSignature -LiteralPath $msi
    if ($signature.Status -ne 'Valid' -or $null -eq $signature.SignerCertificate) {{ throw 'TightVNC installer has no valid Authenticode signature.' }}
    $publisher = $signature.SignerCertificate.GetNameInfo([System.Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false)
    if (@('GlavSoft LLC', 'GlavSoft LLC.', 'GlavSoft OOO') -notcontains $publisher) {{ throw 'TightVNC installer publisher is not trusted.' }}
    $installed = Start-Process "$env:SystemRoot\System32\msiexec.exe" -Wait -PassThru -ArgumentList "/i `"$msi`" /quiet /norestart ADDLOCAL=Server SERVER_REGISTER_AS_SERVICE=1 SERVER_ADD_FIREWALL_EXCEPTION=0 SET_USEVNCAUTHENTICATION=1 VALUE_OF_USEVNCAUTHENTICATION=1 SET_PASSWORD=1 VALUE_OF_PASSWORD={vnc} SET_ALLOWLOOPBACK=1 VALUE_OF_ALLOWLOOPBACK=1 SET_LOOPBACKONLY=1 VALUE_OF_LOOPBACKONLY=1"
    if ($installed.ExitCode -notin @(0, 3010)) {{ throw 'TightVNC installation failed.' }}
}} finally {{
    Remove-Item -LiteralPath $staging -Recurse -Force
}}
$password = '{password}'
net user Administrator $password
if ($LASTEXITCODE -ne 0) {{ throw 'Could not configure the guest account.' }}
$winlogon = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
Set-ItemProperty $winlogon -Name AutoAdminLogon -Value '1'
Set-ItemProperty $winlogon -Name DefaultUserName -Value 'Administrator'
Set-ItemProperty $winlogon -Name DefaultPassword -Value $password
powercfg /change monitor-timeout-ac 0
Restart-Computer -Force
</powershell>
"#)
}

impl Aws {
    pub fn connected() -> Result<Self, String> {
        let settings = super::settings();
        let region = settings.aws_region.ok_or("Connect AWS in Settings → Cloud first.")?;
        Ok(Aws { profile: settings.aws_profile, region })
    }
    pub fn command(&self, args: &[&str]) -> Result<Command, String> {
        let aws = crate::token_saver::find_tool("aws").ok_or("The AWS CLI is not installed. Install it (brew install awscli), sign in with `aws login`, then connect again.")?;
        let mut command = Command::new(aws);
        command.args(args).args(["--region", &self.region, "--output", "json", "--no-cli-pager"])
            .env("AWS_PAGER", "").env("AWS_IGNORE_CONFIGURED_ENDPOINT_URLS", "true");
        if let Some(profile) = &self.profile {
            command.args(["--profile", profile]);
        }
        Ok(command)
    }
    pub fn call(&self, args: &[&str], timeout: Duration) -> Result<Value, String> {
        let out = run_process(self.command(args)?, "AWS", 4 << 20, timeout)?;
        if !out.status.success() {
            return Err(cli_error(&String::from_utf8_lossy(&out.stderr.bytes)));
        }
        let text = String::from_utf8_lossy(&out.stdout.bytes);
        if text.trim().is_empty() { Ok(Value::Null) } else { serde_json::from_str(&text).map_err(|_| "The AWS CLI returned unreadable output.".into()) }
    }

    pub fn identity(&self) -> Result<String, String> {
        let who = self.call(&["sts", "get-caller-identity"], QUICK)?;
        Ok(format!("{} in {}", who["Arn"].as_str().unwrap_or("unknown").rsplit('/').next().unwrap_or("unknown"), self.region))
    }

    /// The IAM role and instance profile that let SSM manage sandboxes. Created once.
    pub fn ensure_profile(&self) -> Result<(), String> {
        if self.call(&["iam", "get-instance-profile", "--instance-profile-name", PROFILE], QUICK).is_ok() {
            return Ok(());
        }
        let trust = json!({"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}).to_string();
        if let Err(error) = self.call(&["iam", "create-role", "--role-name", ROLE, "--description", "Lets Systems Manager manage PandaCode sandboxes", "--assume-role-policy-document", &trust], QUICK) {
            if !error.contains("EntityAlreadyExists") && !error.contains("already exists") { return Err(error); }
        }
        self.call(&["iam", "attach-role-policy", "--role-name", ROLE, "--policy-arn", "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"], QUICK)?;
        self.call(&["iam", "create-instance-profile", "--instance-profile-name", PROFILE], QUICK)?;
        self.call(&["iam", "add-role-to-instance-profile", "--instance-profile-name", PROFILE, "--role-name", ROLE], QUICK)?;
        // New instance profiles take a few seconds to become usable by EC2.
        std::thread::sleep(Duration::from_secs(12));
        Ok(())
    }

    pub fn machine(instance: &Value) -> Machine {
        let tag = |key: &str| instance["Tags"].as_array().and_then(|tags| tags.iter().find(|t| t["Key"] == key)).and_then(|t| t["Value"].as_str()).map(str::to_string);
        let id = instance["InstanceId"].as_str().unwrap_or_default();
        let state = match instance["State"]["Name"].as_str().unwrap_or("unknown") {
            "pending" => "starting",
            "shutting-down" | "stopping" => "stopping",
            "terminated" => "deleted",
            other => other,
        };
        let os = tag("pandacode-os").unwrap_or_else(|| if instance["Platform"] == "windows" { "windows".into() } else { "linux".into() });
        Machine {
            id: format!("aws:{id}"),
            provider: "aws".into(),
            name: tag("Name").unwrap_or_else(|| id.to_string()),
            state: state.into(),
            detail: format!("AWS EC2 · {} · {}", if os == "windows" { "Windows Server" } else { "Ubuntu" }, instance["InstanceType"].as_str().unwrap_or("")),
            os,
            desktop: true,
        }
    }
    pub fn list(&self) -> Result<Vec<Machine>, String> {
        let found = self.call(&["ec2", "describe-instances", "--filters", "Name=tag:pandacode,Values=sandbox", "Name=instance-state-name,Values=pending,running,stopping,stopped"], QUICK)?;
        Ok(found["Reservations"].as_array().into_iter().flatten().flat_map(|r| r["Instances"].as_array().cloned().unwrap_or_default()).map(|i| Self::machine(&i)).collect())
    }

    /// Launch a desktop and return it with the VNC password to keep in the keychain.
    pub fn create(&self, os: &str, name: &str) -> Result<(Machine, String), String> {
        let flavor = flavor(os)?;
        self.ensure_profile()?;
        let image = self.call(&["ssm", "get-parameter", "--name", flavor.parameter], QUICK)?["Parameter"]["Value"].as_str().map(str::to_string).ok_or("AWS did not report a current image for this OS.")?;
        let password = format!("Bb{}!7", &uuid::Uuid::new_v4().simple().to_string()[..18]);
        let user_data = if os == "windows" { windows_user_data(&password) } else { linux_user_data(&password) };
        let file = UserDataFile::create(&user_data)?;
        let tags = format!("ResourceType=instance,Tags=[{{Key=Name,Value={name}}},{{Key=pandacode,Value=sandbox}},{{Key=pandacode-os,Value={os}}}]");
        // Ubuntu and Windows Server images both boot from /dev/sda1.
        let disk = format!("DeviceName=/dev/sda1,Ebs={{VolumeSize={},VolumeType=gp3,DeleteOnTermination=true}}", flavor.disk_gb);
        let launched = self.call(&[
            "ec2", "run-instances", "--image-id", &image, "--instance-type", flavor.instance_type, "--count", "1",
            "--iam-instance-profile", &format!("Name={PROFILE}"), "--user-data", &format!("file://{}", file.path.display()),
            "--tag-specifications", &tags, "--metadata-options", "HttpTokens=required,HttpEndpoint=enabled",
            "--block-device-mappings", &disk, "--instance-initiated-shutdown-behavior", "stop",
        ], Duration::from_secs(180));
        drop(file);
        let launched = launched.map_err(|e| if e.contains("VPCIdNotSpecified") || e.contains("No default VPC") { "This region has no default VPC. Create one (VPC console → Actions → Create default VPC) or pick another region.".into() } else { e })?;
        let instance = launched["Instances"][0].clone();
        Ok((Self::machine(&instance), password))
    }

    pub fn power(&self, id: &str, action: &str) -> Result<(), String> {
        let verb = match action { "start" => "start-instances", "stop" | "suspend" => "stop-instances", "delete" => "terminate-instances", _ => return Err(format!("AWS desktops can start, stop or be deleted, not {action}.")) };
        self.call(&["ec2", verb, "--instance-ids", id], QUICK).map(|_| ())
    }

    /// Run a command with SSM Run Command and wait for it.
    pub fn exec(&self, id: &str, windows: bool, command: &str, timeout: Duration) -> Result<(Option<i32>, String), String> {
        let parameters = json!({"commands": [command], "executionTimeout": [timeout.as_secs().to_string()]}).to_string();
        let sent = self.call(&["ssm", "send-command", "--instance-ids", id, "--document-name", if windows { "AWS-RunPowerShellScript" } else { "AWS-RunShellScript" }, "--parameters", &parameters, "--timeout-seconds", "60"], QUICK)
            .map_err(|e| if e.contains("InvalidInstanceId") { "This instance is not ready for commands yet (Systems Manager has not registered it). Wait a minute after it starts.".into() } else { e })?;
        let command_id = sent["Command"]["CommandId"].as_str().ok_or("AWS did not accept the command.")?.to_string();
        let deadline = std::time::Instant::now() + timeout + Duration::from_secs(60);
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let result = self.call(&["ssm", "get-command-invocation", "--command-id", &command_id, "--instance-id", id], QUICK);
            match result {
                Ok(result) if matches!(result["Status"].as_str(), Some("Pending" | "InProgress" | "Delayed")) => {}
                Ok(result) => {
                    let mut output = result["StandardOutputContent"].as_str().unwrap_or_default().to_string();
                    if let Some(stderr) = result["StandardErrorContent"].as_str().filter(|s| !s.is_empty()) {
                        output.push_str(&format!("\n[stderr]\n{stderr}"));
                    }
                    let code = result["ResponseCode"].as_i64().filter(|c| *c >= 0).map(|c| c as i32);
                    return Ok((code, output));
                }
                Err(error) if error.contains("InvocationDoesNotExist") => {}
                Err(error) => return Err(error),
            }
            if std::time::Instant::now() > deadline {
                return Err("The command did not finish in time.".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_cli_errors_and_expired_logins() {
        assert_eq!(cli_error("\nAn error occurred (UnauthorizedOperation) when calling the RunInstances operation: You are not authorized.\n"), "AWS: You are not authorized.");
        assert!(cli_error("Unable to locate credentials. You can configure credentials by running \"aws login\".").contains("aws login"));
        assert!(cli_error("aws: [ERROR]: Token has expired and refresh failed").contains("expired"));
    }
    #[test]
    fn reads_instances_into_machines() {
        let instance = json!({"InstanceId":"i-0abc","InstanceType":"t3.large","Platform":"windows","State":{"Name":"pending"},"Tags":[{"Key":"Name","Value":"build-box"},{"Key":"pandacode-os","Value":"windows"}]});
        let machine = Aws::machine(&instance);
        assert_eq!((machine.id.as_str(), machine.name.as_str(), machine.state.as_str(), machine.os.as_str()), ("aws:i-0abc", "build-box", "starting", "windows"));
        assert!(machine.desktop && machine.detail.contains("Windows Server"));
    }
    #[test]
    fn desktops_keep_vnc_on_localhost_without_a_vnc_firewall_exception() {
        let linux = linux_user_data("Bbsecret123!7");
        assert!(linux.contains("-localhost yes") && linux.contains("vncpasswd -f") && linux.contains("'Bbsecret123!7'"));
        let windows = windows_user_data("Bbsecret123!7");
        assert!(windows.contains("VALUE_OF_LOOPBACKONLY=1") && windows.contains("SERVER_ADD_FIREWALL_EXCEPTION=0") && windows.contains("AutoAdminLogon"));
        assert!(windows.contains("VALUE_OF_PASSWORD=Bbsecret"), "TightVNC uses the first eight characters");
        assert_eq!(flavor("linux").unwrap().vnc_port, 5901);
        assert!(flavor("macos").is_err());
    }

    #[test]
    fn user_data_file_is_private_exclusive_and_removed_after_use() {
        let dir = std::env::temp_dir().join(format!("pandacode-user-data-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("user-data.txt");
        {
            let file = UserDataFile::create_at(path.clone(), "fixture-password").unwrap();
            assert_eq!(std::fs::read_to_string(&file.path).unwrap(), "fixture-password");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(std::fs::metadata(&file.path).unwrap().permissions().mode() & 0o777, 0o600);
            }
        }
        assert!(!path.exists());
        std::fs::write(&path, "keep-existing-file").unwrap();
        assert!(UserDataFile::create_at(path.clone(), "replacement").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep-existing-file");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn user_data_file_is_removed_when_the_caller_fails() {
        let path = std::env::temp_dir().join(format!("pandacode-user-data-error-{}", uuid::Uuid::new_v4()));
        let failed: Result<(), String> = {
            let _file = UserDataFile::create_at(path.clone(), "fixture-password").unwrap();
            Err("The simulated AWS request failed.".into())
        };
        assert!(failed.is_err());
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn user_data_file_never_follows_or_removes_an_existing_symlink() {
        let dir = std::env::temp_dir().join(format!("pandacode-user-data-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let target = dir.join("target.txt");
        let path = dir.join("user-data.txt");
        std::fs::write(&target, "keep-target").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(UserDataFile::create_at(path.clone(), "replacement").is_err());
        assert!(std::fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep-target");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn linux_user_data_disables_inherited_tracing_before_secrets() {
        let generated = linux_user_data("fixture-secret");
        let prefix = generated.split("export DEBIAN_FRONTEND").next().unwrap();
        // Only exercise the prologue and an inert printf, never provisioning.
        let probe = format!("{prefix}\nprintf '%s' 'fixture-secret' >/dev/null\n");
        let output = Command::new("bash").args(["-x", "-c", &probe]).output().unwrap();
        assert!(output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("fixture-secret"));
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn windows_verifies_publisher_before_install_and_cleans_up_before_autologon() {
        let script = windows_user_data("Bbfixture123!7");
        let signature = script.find("Get-AuthenticodeSignature -LiteralPath $msi").unwrap();
        let valid = script.find("$signature.Status -ne 'Valid'").unwrap();
        let publisher = script.find("-notcontains $publisher").unwrap();
        let install = script.find("Start-Process").unwrap();
        let cleanup = script.find("Remove-Item -LiteralPath $staging -Recurse -Force").unwrap();
        let autologon = script.find("Set-ItemProperty $winlogon -Name AutoAdminLogon").unwrap();
        assert!(signature < valid && valid < publisher && publisher < install);
        assert!(install < cleanup && cleanup < autologon);
        assert!(script.contains("} finally {"));
        assert!(script.contains("'GlavSoft LLC', 'GlavSoft LLC.', 'GlavSoft OOO'"));
        assert!(script.contains("SetAccessRuleProtection($true, $false)"));
        assert!(script.contains("-MaximumRedirection 0 -TimeoutSec 120"));
    }
    #[test]
    fn the_cli_uses_the_chosen_profile_and_region() {
        if crate::token_saver::find_tool("aws").is_none() { return; }
        let command = Aws { profile: Some("work".into()), region: "eu-west-1".into() }.command(&["sts", "get-caller-identity"]).unwrap();
        let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["sts", "get-caller-identity", "--region", "eu-west-1", "--output", "json", "--no-cli-pager", "--profile", "work"]);
        assert!(command.get_envs().any(|(key, value)| key == "AWS_IGNORE_CONFIGURED_ENDPOINT_URLS" && value.is_some_and(|value| value == "true")));
    }
}
