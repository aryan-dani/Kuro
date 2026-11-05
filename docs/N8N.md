# Self-hosted n8n for voice workflows

Kuro on the laptop calls n8n on `kuro-server`. The hub stays on port 18200 for desk and news. Mail triage stays on n8n Cloud. See [CLOUD_INBOX.md](CLOUD_INBOX.md).

The server LAN address is `192.168.1.240` only. Kuro does not use that address. It uses Tailscale `100.78.60.50`, same as the hub.

The n8n container is already running. Hello Kuro is imported and active. `KURO_N8N_TOKEN` in the laptop `.env` matches the webhook credential. Open `http://100.78.60.50:5678` once and create the owner account. Restart Kuro so it loads this code.

Webhooks are reachable only on localhost and Tailscale. Do not publish port 5678 on `0.0.0.0`, and do not expose the editor to the internet.

## Run n8n

On `kuro-server`, from a copy of [deploy/n8n/docker-compose.yml](../deploy/n8n/docker-compose.yml):

```bash
sudo mkdir -p /srv/n8n
sudo chown "$USER" /srv/n8n
cp docker-compose.yml /srv/n8n/
umask 077
# N8N_ENCRYPTION_KEY is required by n8n. Keep this file on the server only.
printf 'N8N_ENCRYPTION_KEY=%s\n' "$(openssl rand -hex 32)" > /srv/n8n/.env
cd /srv/n8n
docker compose up -d
```

Editor: `http://100.78.60.50:5678` from a machine on your tailnet, or `http://127.0.0.1:5678` on the server. Create the owner account on first open.

## Build the hello workflow

Import [examples/n8n/hello_kuro.json](../examples/n8n/hello_kuro.json) (Workflows, Import from File). If import fails, build it by hand:

1. Add a Webhook node. Method POST. Path `hello-kuro`. Respond `Using Respond to Webhook` node.
2. Authentication: Header Auth. Create a credential whose header name is `X-Kuro-Token` and whose value is the same string as `KURO_N8N_TOKEN` on the laptop.
3. Attach that credential to the Webhook node.
4. Add a Respond to Webhook node. Respond with JSON: `{"reply":"Hello from n8n"}`.
5. Connect Webhook to Respond to Webhook.
6. Save, then activate the workflow. Production URL is `http://100.78.60.50:5678/webhook/hello-kuro`. The `/webhook-test/` URL only works while you are listening in the editor. Kuro always calls `/webhook/`.

## Laptop config

In `.env` (never commit it):

```
KURO_N8N_BASE_URL=http://100.78.60.50:5678/webhook
KURO_N8N_TOKEN=paste-the-header-value
```

Copy [n8n_workflows.example.json](../n8n_workflows.example.json) to `%LOCALAPPDATA%\Kuro\n8n_workflows.json` and edit that copy. The repo file is only a sample.

Restart Kuro after changing `.env`. The phrase router re-reads the registry on every command, so new workflows work without a restart. The LLM tool description is built at startup, so it sees new names only after a restart.

To add a workflow: create and activate it in n8n, then add one object under `workflows` in the local JSON. `mode` `sync` waits up to 8 seconds and speaks `reply`. `mode` `async` returns as soon as n8n accepts the POST and Kuro says it started. `takes_input` true sends the words after the alias as `{"text": "...", "source": "kuro"}`.

Say `run hello workflow` or `n8n hello`. Say `log idea build the reader` to pass `build the reader` as the text.

If the base URL or token is empty, Kuro says `n8n is not configured.` Desk commands still work.

## Check by hand

1. Say the hello workflow and hear "Hello from n8n".
2. Stop the n8n container. Say the hello workflow again and hear that Kuro cannot reach n8n. Say "desk off" or "what's the news" and confirm those still answer.
3. Add a new entry to `%LOCALAPPDATA%\Kuro\n8n_workflows.json` without restarting Kuro, then trigger it by voice.
4. After each of these, hold Ctrl+Alt+Space again and confirm the hotkey is not stuck busy.
