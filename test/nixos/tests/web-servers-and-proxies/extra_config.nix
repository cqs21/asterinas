{ config, lib, pkgs, ... }:
let
  openresty_conf = pkgs.writeTextFile {
    name = "openresty.conf";
    text = ''
      worker_processes  1;
      error_log /tmp/openresty/error.log;
      events {
          worker_connections 1024;
      }
      http {
          server {
              listen 10.0.2.15:8003;
              location / {
                  default_type text/html;
                  content_by_lua_block {
                      ngx.say("Hello from Openresty")
                  }
              }
          }
      }
    '';
  };
in {
  environment.systemPackages = with pkgs; [ apacheHttpd caddy nginx openresty ];
  environment.sessionVariables = {
    HTTPD_ROOT = "${pkgs.apacheHttpd}";
    NGINX_ROOT = "${pkgs.nginx}";
  };
  environment.loginShellInit = ''
    [ ! -e /tmp/openresty.conf ] && ln -s ${openresty_conf} /tmp/openresty.conf
  '';
}
