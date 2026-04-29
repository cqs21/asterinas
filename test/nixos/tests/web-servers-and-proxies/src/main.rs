// SPDX-License-Identifier: MPL-2.0

//! The test suite for web servers and proxies applications on Asterinas NixOS.

use nixos_test_framework::*;

nixos_test_main!();

// ============================================================================
// Web Servers
// ============================================================================

#[nixos_test]
fn httpd_server(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd(r#"cp -rL "$HTTPD_ROOT" /tmp/httpd"#)?;
    nixos_shell
        .run_cmd(r"sed -i 's/^Listen 80$/Listen 10.0.2.15:8000/' /tmp/httpd/conf/httpd.conf")?;
    nixos_shell.run_cmd("sed -i 's/^User daemon$/User apache/' /tmp/httpd/conf/httpd.conf")?;
    nixos_shell.run_cmd("sed -i 's/^Group daemon$/Group apache/' /tmp/httpd/conf/httpd.conf")?;
    nixos_shell.run_cmd("groupadd -r apache 2>/dev/null")?;
    nixos_shell.run_cmd("useradd -r -g apache -s /sbin/nologin apache 2>/dev/null")?;

    nixos_shell.run_cmd("httpd -f /tmp/httpd/conf/httpd.conf")?;
    nixos_shell.run_cmd_and_expect("curl http://10.0.2.15:8000", "It works!")?;
    nixos_shell.run_cmd("httpd -f /tmp/httpd/conf/httpd.conf -k stop")?;
    Ok(())
}

#[nixos_test]
fn caddy_server(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("mkdir -p /tmp/caddy")?;
    nixos_shell.run_cmd("cd /tmp/caddy && echo 'Hello from Caddy' > index.html")?;

    nixos_shell
        .run_cmd("caddy file-server --listen 10.0.2.15:8001 --browse > /tmp/caddy.log 2>&1 &")?;
    nixos_shell.run_cmd("sleep 3")?;
    nixos_shell.run_cmd_and_expect("curl http://10.0.2.15:8001", "Hello from Caddy")?;
    nixos_shell.run_cmd("pkill caddy")?;
    Ok(())
}

#[nixos_test]
fn nginx_server(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd(r#"cp -rL "$NGINX_ROOT" /tmp/nginx"#)?;
    nixos_shell.run_cmd(
        r"sed -i 's/^\(\s*listen\s*\)80;/\110.0.2.15:8002;/' /tmp/nginx/conf/nginx.conf",
    )?;
    nixos_shell.run_cmd("mkdir -p /var/log/nginx")?;

    nixos_shell.run_cmd("nginx -c /tmp/nginx/conf/nginx.conf")?;
    nixos_shell.run_cmd_and_expect("curl http://10.0.2.15:8002", "Welcome to nginx!")?;
    nixos_shell.run_cmd("nginx -s stop")?;
    Ok(())
}

#[nixos_test]
fn openresty_server(nixos_shell: &mut Session) -> Result<(), Error> {
    nixos_shell.run_cmd("mkdir /tmp/openresty")?;
    nixos_shell.run_cmd("mkdir -p /var/log/nginx")?;

    nixos_shell.run_cmd("openresty -p /tmp/openresty -c /tmp/openresty.conf")?;
    nixos_shell.run_cmd_and_expect("curl http://10.0.2.15:8003", "Hello from Openresty")?;
    nixos_shell.run_cmd("openresty -s stop")?;
    Ok(())
}
