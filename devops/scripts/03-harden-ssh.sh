#!/usr/bin/env bash
set -euo pipefail

# Phase 3: stop the SSH brute force (77k failed password attempts in 7 days).
#
# The filename matters. /etc/ssh/sshd_config has `Include sshd_config.d/*.conf`
# on line 12 and sshd keeps the FIRST value it sees for a keyword. The box
# already has:
#     50-cloud-init.conf        PasswordAuthentication yes
#     60-cloudimg-settings.conf PasswordAuthentication no
# 50 sorts first, so password auth is on and the 60 file is dead weight. A
# drop-in named 99-* would be read last and change nothing — hence 00-*.
#
# cloud-init rewrites 50-cloud-init.conf from `ssh_pwauth`, so that is pinned
# too, otherwise a rebuild or reboot can quietly turn password auth back on.
#
# This script never closes your current session: it validates with `sshd -t`
# before applying and uses `reload`, not `restart`, so live sessions survive.
# It also refuses to run if root has no authorized key.
#
# Usage: ./devops/scripts/03-harden-ssh.sh [ssh-host]   (default: myserver)

HOST="${1:-myserver}"
say() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

say "Preflight: a usable key must exist before passwords are switched off"
ssh "$HOST" bash -euo pipefail -s <<'GUARD'
keys=$(grep -cvE '^\s*(#|$)' /root/.ssh/authorized_keys 2>/dev/null || echo 0)
echo "  authorized keys for root: $keys"
[ "$keys" -ge 1 ] || { echo "ABORT: no authorized key — disabling passwords would lock you out"; exit 1; }
GUARD

say "Applying sshd hardening"
ssh "$HOST" bash -euo pipefail -s <<'SSHD'
cp -a /etc/ssh/sshd_config /root/backups/sshd_config.bak-$(date +%Y%m%d-%H%M%S) 2>/dev/null || {
  mkdir -p /root/backups
  cp -a /etc/ssh/sshd_config /root/backups/sshd_config.bak-$(date +%Y%m%d-%H%M%S)
}

cat > /etc/ssh/sshd_config.d/00-hardening.conf <<'CONF'
# Sorts before 50-cloud-init.conf, which sets PasswordAuthentication yes.
# sshd honours the first value it reads, so this file wins.
PasswordAuthentication no
PermitRootLogin prohibit-password
KbdInteractiveAuthentication no
ChallengeResponseAuthentication no
PermitEmptyPasswords no
MaxAuthTries 3
X11Forwarding no
CONF
chmod 644 /etc/ssh/sshd_config.d/00-hardening.conf

# Keep cloud-init from regenerating 50-cloud-init.conf with passwords enabled.
mkdir -p /etc/cloud/cloud.cfg.d
cat > /etc/cloud/cloud.cfg.d/99-disable-password-auth.cfg <<'CONF'
ssh_pwauth: false
CONF

echo "-- validating config"
sshd -t
echo "   syntax OK"

systemctl reload ssh
sleep 1

echo "-- effective settings now:"
sshd -T | grep -E '^(passwordauthentication|permitrootlogin|kbdinteractiveauthentication|permitemptypasswords|maxauthtries)' | sed 's/^/   /'
SSHD

say "Proving key auth still works on a brand-new connection"
# The reload above kept the existing session alive; this opens a fresh one. If
# this fails, the previous session is still open to undo the change.
ssh -o BatchMode=yes -o ConnectTimeout=10 "$HOST" 'echo "   new session OK as $(whoami)@$(hostname)"'

say "Installing fail2ban"
ssh "$HOST" bash -euo pipefail -s <<'F2B'
DEBIAN_FRONTEND=noninteractive apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq fail2ban >/dev/null

cat > /etc/fail2ban/jail.local <<'CONF'
[DEFAULT]
# Ignore nothing by default beyond loopback; add your own IP here if you want a
# guaranteed escape hatch.
ignoreip = 127.0.0.1/8 ::1
bantime  = 1h
findtime = 10m
maxretry = 3
backend  = systemd

[sshd]
enabled = true
CONF

systemctl enable --now fail2ban >/dev/null
sleep 3
fail2ban-client status sshd | sed 's/^/   /'
F2B

say "Enabling the firewall"
ssh "$HOST" bash -euo pipefail -s <<'UFW'
# Order matters: allow SSH before enabling, or this session dies with it.
ufw --force reset >/dev/null
ufw default deny incoming >/dev/null
ufw default allow outgoing >/dev/null
ufw allow 22/tcp    comment 'ssh'          >/dev/null
ufw allow 80/tcp    comment 'http'         >/dev/null
ufw allow 443       comment 'https + h3'   >/dev/null
ufw allow 51820/udp comment 'wireguard'    >/dev/null
ufw --force enable >/dev/null
ufw status verbose | sed 's/^/   /'

echo
echo "   NOTE: Docker publishes ports below ufw, so ufw does not gate container"
echo "   ports. That is why Postgres and Redis were unpublished in phase 1 —"
echo "   that, not ufw, is what actually closed them."
UFW

say "Confirming exposure"
ssh "$HOST" 'ss -tulpn | grep -E "LISTEN|UNCONN" | grep -vE "127.0.0.(1|53|54)" | sed "s/^/   /"'

cat <<EOF

SSH hardening done. Password logins are off; only your ed25519 key works.

If you ever need to undo it:
  ssh $HOST 'rm /etc/ssh/sshd_config.d/00-hardening.conf && systemctl reload ssh'

Watch the attempts stop:
  ssh $HOST 'journalctl -u ssh -f'
  ssh $HOST 'fail2ban-client status sshd'
EOF
