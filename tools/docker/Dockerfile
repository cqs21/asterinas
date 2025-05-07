# SPDX-License-Identifier: MPL-2.0

ARG BASE_VERSION
FROM asterinas/osdk:${BASE_VERSION} AS build-base

SHELL ["/bin/bash", "-c"]

ARG DEBIAN_FRONTEND=noninteractive

#= Install packages for Docker building ====================================

# Please keep the list sorted by name
RUN apt update && apt-get install -y --no-install-recommends \
    apache2-utils \
    clang \
    cmake \
    curl \
    gnupg \
    libevent-dev \
    libslirp-dev \
    libssl-dev \
    jq \
    tcl-dev \
    unzip \
    zip
RUN sh <(curl -L https://nixos.org/nix/install) --daemon --yes \
    && . /etc/profile.d/nix.sh \
    && nix-env -iA nixpkgs.nixfmt \
    && nix-collect-garbage

#= Download dependency =====================================================

WORKDIR /opt/linux_binary_cache
RUN wget https://raw.githubusercontent.com/asterinas/linux_vdso/2a6d2db/vdso64.so -O vdso64.so
RUN wget https://raw.githubusercontent.com/asterinas/linux_binary_cache/14598b6/vmlinuz-5.15.0-105 \
        -O vmlinuz

#= Install applications =======================================================

FROM build-base AS build-applications

# Install SQLite
WORKDIR /root
RUN apt-get install -y sqlite sqlite3
RUN wget https://www.sqlite.org/2024/sqlite-amalgamation-3460100.zip \
    && unzip sqlite-amalgamation-3460100.zip \
    && cd sqlite-amalgamation-3460100 \
    && gcc -g -shared -fPIC -c sqlite3.c \
    && gcc -g -shared -fPIC -o libsqlite3.so sqlite3.o \
    && mv ./libsqlite3.so /lib/x86_64-linux-gnu/ \
    && mv ./sqlite3.h /usr/include/x86_64-linux-gnu/ \
    && mv ./sqlite3ext.h /usr/include/x86_64-linux-gnu/

RUN rm -rf sqlite-amalgamation-3460100.zip \
    sqlite-amalgamation-3460100

# Install SQLite-speedtest
WORKDIR /root
RUN git clone --branch version-3.46.1 https://github.com/sqlite/sqlite.git
RUN cd sqlite \
    && mkdir bld \
    && cd bld \
    && ../configure --enable-all \
    && make speedtest1 \
    && cp ./speedtest1 /usr/local

RUN rm -rf sqlite

# Instal LevelDB 1.23
WORKDIR /root
RUN mkdir -p /usr/local/leveldb/benchmark/
RUN git clone -b 1.23 --recurse-submodules https://github.com/google/leveldb.git \
    && cd leveldb \
    && mkdir -p build \
    && cd build \
    && cmake -DCMAKE_BUILD_TYPE=Release .. \
    && cmake --build . \
    && make install \
    && mv ./db_bench /usr/local/leveldb/benchmark/ \
    && mv ./db_bench_sqlite3 /usr/local/leveldb/benchmark/

RUN rm -rf 1.23.tar.gz \
    leveldb-1.23

# Install Redis-7.0.15
WORKDIR /root
RUN wget https://download.redis.io/releases/redis-7.0.15.tar.gz \
    && tar -xzvf redis-7.0.15.tar.gz \
    && cd redis-7.0.15 \
    && make -j \
    && make PREFIX=/usr/local/redis install

RUN rm -rf redis-7.0.15.tar.gz \
    redis-7.0.15

# Install Nginx only with http enabled
WORKDIR /root
RUN wget https://nginx.org/download/nginx-1.26.2.tar.gz \
    && tar -xzvf nginx-1.26.2.tar.gz \
    && cd nginx-1.26.2 \
    && ./configure --with-cc-opt="-O2" --with-ld-opt="-static" --without-select_module --without-poll_module \
    --without-quic_bpf_module --without-http_charset_module --without-http_gzip_module --without-http_ssi_module \
    --without-http_userid_module --without-http_access_module --without-http_auth_basic_module --without-http_mirror_module \
    --without-http_geo_module --without-http_map_module --without-http_split_clients_module --without-http_referer_module \
    --without-http_rewrite_module --without-http_proxy_module --without-http_fastcgi_module --without-http_uwsgi_module \
    --without-http_scgi_module --without-http_grpc_module --without-http_memcached_module --without-http_limit_conn_module \
    --without-http_limit_req_module --without-http_empty_gif_module --without-http_browser_module --without-http_upstream_hash_module \
    --without-http_upstream_ip_hash_module --without-http_upstream_least_conn_module --without-http_upstream_random_module \
    --without-http_upstream_keepalive_module --without-http_upstream_zone_module --without-http-cache --without-mail_pop3_module \
    --without-mail_imap_module --without-mail_smtp_module --without-stream_limit_conn_module --without-stream_access_module \
    --without-stream_geo_module --without-stream_map_module --without-stream_split_clients_module --without-stream_return_module \
    --without-stream_pass_module --without-stream_set_module --without-stream_upstream_hash_module --without-stream_upstream_least_conn_module \
    --without-stream_upstream_random_module --without-stream_upstream_zone_module --without-pcre --without-pcre2

WORKDIR /root/nginx-1.26.2
RUN make -j \
    && make install

WORKDIR /root
RUN rm -rf nginx-1.26.2.tar.gz \
    nginx-1.26.2

# Install Memcached v1.6.32
WORKDIR /root
RUN wget https://www.memcached.org/files/memcached-1.6.32.tar.gz \
    && tar -xzvf memcached-1.6.32.tar.gz \
    && cd memcached-1.6.32 \
    && ./configure --prefix=/usr/local/memcached \
    && make -j \
    && make install

RUN rm -rf memcached-1.6.32.tar.gz \
    memcached-1.6.32

# Install Apache httpd v2.4.62
WORKDIR /root
RUN apt-get install -y --no-install-recommends libapr1-dev libaprutil1-dev libpcre2-dev
RUN wget https://dlcdn.apache.org/httpd/httpd-2.4.63.tar.bz2 \
    && tar xvf httpd-2.4.63.tar.bz2 \
    && cd httpd-2.4.63 \
    && ./configure \
    --disable-authn-file \
    --disable-authz-groupfile \
    --disable-authz-user \
    --disable-access-compat \
    --disable-auth-basic \
    --disable-reqtimeout \
    --disable-filter \
    --disable-charset-lite \
    --disable-mime \
    --disable-log-config \
    --disable-env \
    --disable-headers \
    --disable-setenvif \
    --disable-version \
    --disable-status \
    --disable-autoindex \
    --disable-dir \
    --disable-alias \
    && make -j \
    && make install

RUN rm -rf httpd-2.4.63.tar.bz2 \
    httpd-2.4.63

# Install wrk
WORKDIR /root
RUN git clone https://github.com/wg/wrk.git \
    && cd wrk \
    && make -j \
    && mkdir -p /usr/local/wrk \
    && cp wrk /usr/local/wrk

RUN rm -rf wrk

#= The final stages to produce the Asterinas development image ====================

FROM build-base

# Install all Asterinas dependent packages
RUN apt update && apt-get install -y --no-install-recommends \
    bridge-utils \
    clang-format       `# formatting general tests` \
    cpio \
    cpuid \
    exfatprogs \
    file \
    grub-efi-amd64-bin \
    grub-efi-amd64-dbg \
    iptables \
    iproute2 \
    libnl-3-dev         `# dependency for netlink socket` \
    libnl-route-3-dev   `# dependency for netlink route socket` \
    net-tools \
    openssh-server \
    pkg-config \
    socat \
    strace \
    sudo \
    unzip \
    vim \
    zip
# Clean apt cache
RUN apt clean && rm -rf /var/lib/apt/lists/*

# Install applications built from the previous stages
COPY --from=build-applications /usr/local/redis /usr/local/redis
COPY --from=build-applications /usr/local/nginx /usr/local/nginx
COPY --from=build-applications /usr/local/leveldb /usr/local/leveldb
COPY --from=build-applications /usr/local/speedtest1 /usr/local/benchmark/sqlite-speedtest1
COPY --from=build-applications /lib/x86_64-linux-gnu/libsqlite3.so /lib/x86_64-linux-gnu/libsqlite3.so
COPY --from=build-applications /usr/local/memcached /usr/local/memcached
COPY --from=build-applications /usr/local/apache2 /usr/local/apache2
COPY --from=build-applications /usr/local/wrk /usr/local/wrk

# Add the path of Asterinas tools
ENV PATH="/root/asterinas/target/bin:${PATH}"

VOLUME [ "/root/asterinas" ]

WORKDIR /root/asterinas
