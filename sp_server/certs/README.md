# Certificate Generation for sp_server

Steps to generate self-signed TLS certificates on Ubuntu 24.04.

## Prerequisites

Install Node.js 20 LTS and npm:

```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
```

Install OpenSSL:

```bash
sudo apt-get install -y openssl
```

## Update san.cnf

Edit `san.cnf` and set the `CN` and `IP.1` fields to match your machine's IP address:

```
CN = <your-ip-address>

[alt_names]
IP.1 = <your-ip-address>
```

## Generate Certificates

From the `sp_server/certs/` directory:

```bash
openssl req -x509 -nodes -days 825 -newkey rsa:2048 \
  -keyout server.key \
  -out server.crt \
  -config san.cnf \
  -extensions req_ext
```

## Verify

```bash
openssl x509 -in server.crt -text -noout
```

## Update Environment Variables

Update `sp_server/.env`:

```
PUBLIC_CERT_PATH=/home/user/sp/sp_server/certs/server.crt
PRIVATE_KEY_PATH=/home/user/sp/sp_server/certs/server.key
```

Update `sp_axum/.env`:

```
CERT_PATH=/home/user/sp/sp_server/certs/
CERT_NAME=server.crt
KEY_NAME=server.key
```
