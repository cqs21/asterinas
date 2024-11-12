#!/bin/sh

HOST_IP=10.0.2.15

echo """
user root;

events {
}

http {
    include mime.types;

    server {
        listen ${HOST_IP}:8080;

        location / {
            autoindex on;
        }
    }
}
""" > /usr/local/nginx/conf/nginx.conf

/usr/local/nginx/sbin/nginx
