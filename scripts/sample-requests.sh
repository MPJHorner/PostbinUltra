#!/usr/bin/env bash
# Fire a batch of realistic-looking HTTP requests at a Postbin Ultra capture
# server, useful for exercising the desktop UI and the web UI when you don't
# have a real upstream pumping traffic at it.
#
# Usage:
#   scripts/sample-requests.sh                    # default 25 reqs at :9000
#   scripts/sample-requests.sh -p 7777            # custom port
#   scripts/sample-requests.sh -u http://host:9000 # arbitrary base URL
#   scripts/sample-requests.sh -n 50              # repeat the set ~2x
#   scripts/sample-requests.sh -d 0.1             # 100ms delay between sends
#
# Each request lands as one capture in Postbin. The set covers all common
# verbs, content types (JSON, XML, form, multipart, CSV, octet-stream, plain
# text, HTML, GraphQL), webhook-style payloads, and binary file uploads.

set -euo pipefail

PORT=9000
BASE_URL=""
COUNT=25
DELAY=0.05

usage() {
  sed -n '2,16p' "$0"
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -p|--port) PORT="$2"; shift 2 ;;
    -u|--url)  BASE_URL="$2"; shift 2 ;;
    -n|--count) COUNT="$2"; shift 2 ;;
    -d|--delay) DELAY="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

BASE="${BASE_URL:-http://127.0.0.1:$PORT}"

# Sanity check.
if ! curl -sf -o /dev/null --max-time 2 "$BASE/" ; then
  echo "Postbin doesn't appear to be listening at $BASE" >&2
  echo "Start it first: make run    (or: make desktop)" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ---------- Sample files for upload requests ----------
cat > "$TMP/notes.txt" <<'EOF'
Q3 strategy notes
- migrate billing to the new ledger
- ship mobile push by Aug 1
- review on-call rotation for Sept
EOF

cat > "$TMP/users.csv" <<'EOF'
id,name,email,plan,created_at
1,Alice Chen,alice@example.com,pro,2024-11-02T10:14:00Z
2,Bob Diaz,bob@example.com,starter,2024-12-21T08:31:55Z
3,Carol Eze,carol@example.com,pro,2025-01-08T13:00:12Z
4,Dan Fogg,dan@example.com,enterprise,2025-02-15T22:17:43Z
EOF

# Tiny 1x1 red PNG.
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDAT\x08\x99c\xf8\xcf\xc0\x00\x00\x00\x03\x00\x01[\x88\xc2\xfa\x00\x00\x00\x00IEND\xaeB`\x82' > "$TMP/pixel.png"

# Tiny fake JPEG: SOI marker + random body + EOI marker. Enough to look
# binary in a hex view without dragging in a real image.
{
  printf '\xff\xd8\xff\xe0\x00\x10JFIF\x00\x01\x01\x01\x00H\x00H\x00\x00'
  head -c 256 /dev/urandom
  printf '\xff\xd9'
} > "$TMP/photo.jpg"

# A 32 KB blob to exercise the truncation / size pill in the UI.
head -c 32768 /dev/urandom > "$TMP/random.bin"

# ---------- Helpers ----------
TRACE() {
  # 32 hex chars trace-id + 16 hex chars span-id.
  printf '00-%032x-%016x-01' "$RANDOM$RANDOM$RANDOM" "$RANDOM$RANDOM"
}
IDEMP() { uuidgen 2>/dev/null || python3 -c 'import uuid;print(uuid.uuid4())'; }

# Wrap curl so a single failure doesn't kill the whole batch and so we always
# print a one-line summary.
fire() {
  local label="$1"; shift
  local code
  code=$(curl -sS -o /dev/null -w "%{http_code}" --max-time 5 "$@" || echo "ERR")
  printf "%s  %s\n" "$code" "$label"
  sleep "$DELAY"
}

echo "Firing requests at $BASE (count=$COUNT, delay=${DELAY}s)"
echo "----"

declare -a RUNNERS=(
  r_root_get
  r_users_list
  r_login_json
  r_stripe_webhook
  r_github_webhook
  r_slack_url_verify
  r_contact_form
  r_upload_notes
  r_upload_avatar
  r_create_order
  r_update_user
  r_patch_user
  r_delete_session
  r_soap_order
  r_logs_plain
  r_html_render
  r_csv_import
  r_blob_octet
  r_options_preflight
  r_head_health
  r_graphql_query
  r_twilio_sms
  r_analytics_event
  r_photo_upload_raw
  r_sendgrid_batch
)

# ----- Individual request runners -----
r_root_get() {
  fire "GET    /" -X GET "$BASE/" \
    -H "User-Agent: SamplePoster/1.2 (+https://example.com)" \
    -H "X-Request-Id: $(IDEMP)"
}

r_users_list() {
  fire "GET    /api/v1/users (paged)" -G "$BASE/api/v1/users" \
    --data-urlencode "page=2" \
    --data-urlencode "per_page=25" \
    --data-urlencode "sort=-created_at" \
    --data-urlencode "filter[plan]=pro" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -H "Accept: application/json" \
    -H "Traceparent: $(TRACE)"
}

r_login_json() {
  fire "POST   /api/v1/auth/login" -X POST "$BASE/api/v1/auth/login" \
    -H "Content-Type: application/json" \
    -H "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/537.36" \
    -d '{"email":"matt@matthorner.co.uk","password":"correct horse battery staple","remember":true,"device":{"name":"matt-mbp","os":"darwin"}}'
}

r_stripe_webhook() {
  local sig="t=$(date +%s),v1=fakesignaturefakesignaturefakesignature,v0=oldsig"
  fire "POST   /webhooks/stripe" -X POST "$BASE/webhooks/stripe" \
    -H "Content-Type: application/json" \
    -H "Stripe-Signature: $sig" \
    -H "User-Agent: Stripe/1.0 (+https://stripe.com/docs/webhooks)" \
    -d '{"id":"evt_1NbRZX2eZvKYlo2C","object":"event","api_version":"2024-04-10","created":'"$(date +%s)"',"type":"checkout.session.completed","data":{"object":{"id":"cs_test_a1b2c3","object":"checkout.session","amount_total":4999,"currency":"usd","customer_email":"buyer@example.com","payment_status":"paid","metadata":{"order_id":"ord_8821","plan":"pro_annual","referral":"newsletter_q3"}}},"livemode":false}'
}

r_github_webhook() {
  fire "POST   /webhooks/github (push)" -X POST "$BASE/webhooks/github" \
    -H "Content-Type: application/json" \
    -H "X-GitHub-Event: push" \
    -H "X-GitHub-Delivery: $(IDEMP)" \
    -H "X-Hub-Signature-256: sha256=fake0a8b9c1d2e3f4a5b6c7d8e9f0123456789abcdef0123456789abcdef0123" \
    -H "User-Agent: GitHub-Hookshot/abcd1234" \
    -d '{"ref":"refs/heads/main","before":"a1b2c3d4","after":"e5f6a7b8","repository":{"id":7654321,"name":"postbin-ultra","full_name":"matthorner/postbin-ultra","private":false},"pusher":{"name":"matt","email":"matt@matthorner.co.uk"},"commits":[{"id":"e5f6a7b8","message":"fix: handle empty body","author":{"name":"Matt Horner","email":"matt@matthorner.co.uk"},"added":["src/foo.rs"],"removed":[],"modified":["src/bar.rs"]}]}'
}

r_slack_url_verify() {
  fire "POST   /webhooks/slack" -X POST "$BASE/webhooks/slack" \
    -H "Content-Type: application/json" \
    -H "X-Slack-Request-Timestamp: $(date +%s)" \
    -H "X-Slack-Signature: v0=fake7d4f8b" \
    -d '{"token":"verification_token_xxx","challenge":"3eZbrw1aBm2rZgRNFdxV2595E9CY3gmdALWMmHkvFXO7tYXAYM8P","type":"url_verification"}'
}

r_contact_form() {
  fire "POST   /contact (form)" -X POST "$BASE/contact" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -H "Origin: https://www.example.com" \
    -H "Referer: https://www.example.com/contact" \
    --data-urlencode "name=Jane Doe" \
    --data-urlencode "email=jane@example.com" \
    --data-urlencode "subject=Pricing question" \
    --data-urlencode "message=Hi, do you offer non-profit discounts on the team plan? We're a 12-person org. Thanks!"
}

r_upload_notes() {
  fire "POST   /uploads (multipart text)" -X POST "$BASE/uploads" \
    -F "title=Q3 strategy notes" \
    -F "tags=draft,personal,quarterly" \
    -F "owner_id=42" \
    -F "attachment=@$TMP/notes.txt;type=text/plain"
}

r_upload_avatar() {
  fire "POST   /uploads/avatar (PNG)" -X POST "$BASE/uploads/avatar" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -F "user_id=42" \
    -F "avatar=@$TMP/pixel.png;type=image/png"
}

r_create_order() {
  fire "POST   /api/v1/orders" -X POST "$BASE/api/v1/orders" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -H "Idempotency-Key: $(IDEMP)" \
    -H "Traceparent: $(TRACE)" \
    -d '{"customer_id":"cus_88f1","currency":"usd","items":[{"sku":"BOOK-EFF-RUST","name":"Effective Rust","qty":1,"unit_price":3499},{"sku":"STICKER-PACK","name":"Sticker pack","qty":2,"unit_price":499}],"shipping":{"line1":"100 Pine St","city":"San Francisco","region":"CA","postal_code":"94111","country":"US"},"discount":{"code":"BACK2SCHOOL","type":"percent","value":10}}'
}

r_update_user() {
  fire "PUT    /api/v1/users/42" -X PUT "$BASE/api/v1/users/42" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -H "If-Match: \"e0a1c4\"" \
    -d '{"name":"Matt Horner","email":"matt@matthorner.co.uk","timezone":"America/New_York","preferences":{"newsletter":true,"product_updates":false,"theme":"dark"}}'
}

r_patch_user() {
  fire "PATCH  /api/v1/users/42" -X PATCH "$BASE/api/v1/users/42" \
    -H "Content-Type: application/merge-patch+json" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -d '{"role":"superadmin","preferences":{"theme":"light"}}'
}

r_delete_session() {
  fire "DELETE /api/v1/sessions/abc-123" -X DELETE "$BASE/api/v1/sessions/abc-123" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -H "X-Request-Id: $(IDEMP)"
}

r_soap_order() {
  fire "POST   /soap/orders (XML)" -X POST "$BASE/soap/orders" \
    -H 'Content-Type: text/xml; charset=utf-8' \
    -H 'SOAPAction: "http://example.com/CreateOrder"' \
    -d '<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:o="http://example.com/orders">
  <soap:Header>
    <o:Auth><o:Token>fake-saml-token</o:Token></o:Auth>
  </soap:Header>
  <soap:Body>
    <o:CreateOrder>
      <o:Customer id="C-7781">Acme Corp</o:Customer>
      <o:Items>
        <o:Item sku="A-100" qty="3" unit-price="49.99"/>
        <o:Item sku="B-220" qty="1" unit-price="129.99"/>
      </o:Items>
      <o:Total currency="USD">279.96</o:Total>
    </o:CreateOrder>
  </soap:Body>
</soap:Envelope>'
}

r_logs_plain() {
  local body
  body=$(printf '%s\n%s\n%s\n%s\n' \
    "$(date -u +%FT%TZ) INFO  service=api    started ok pid=4218" \
    "$(date -u +%FT%TZ) WARN  service=cache  miss key=user:42" \
    "$(date -u +%FT%TZ) ERROR service=api    upstream timeout after 5000ms target=billing" \
    "$(date -u +%FT%TZ) INFO  service=worker drained 17 jobs in 1.2s")
  fire "POST   /logs/ingest (plain)" -X POST "$BASE/logs/ingest" \
    -H "Content-Type: text/plain" \
    -H "X-Source: api-gateway" \
    --data-binary "$body"
}

r_html_render() {
  fire "POST   /render/preview (HTML)" -X POST "$BASE/render/preview" \
    -H "Content-Type: text/html; charset=utf-8" \
    --data-binary '<!doctype html><html><head><title>Preview</title></head><body><article class="post"><h1>Welcome</h1><p>This is a <strong>preview</strong> of the rendered article. <a href="https://example.com">Read more</a>.</p><ul><li>Point one</li><li>Point two</li></ul></article></body></html>'
}

r_csv_import() {
  fire "POST   /import/users (CSV)" -X POST "$BASE/import/users" \
    -H "Content-Type: text/csv" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    --data-binary "@$TMP/users.csv"
}

r_blob_octet() {
  fire "POST   /v1/blobs (octet-stream 32k)" -X POST "$BASE/v1/blobs" \
    -H "Content-Type: application/octet-stream" \
    -H "X-Filename: payload.bin" \
    -H "X-Sha256: fakehash" \
    --data-binary "@$TMP/random.bin"
}

r_options_preflight() {
  fire "OPTIONS /api/v1/users/42 (CORS)" -X OPTIONS "$BASE/api/v1/users/42" \
    -H "Origin: https://app.example.com" \
    -H "Access-Control-Request-Method: PATCH" \
    -H "Access-Control-Request-Headers: authorization,content-type,idempotency-key,traceparent"
}

r_head_health() {
  # `--head` (not `-X HEAD`) so curl doesn't wait for a body that HEAD never
  # sends. Otherwise the request still fires but curl reports a timeout.
  fire "HEAD   /healthz" --head "$BASE/healthz" \
    -H "User-Agent: kube-probe/1.30"
}

r_graphql_query() {
  fire "POST   /graphql" -X POST "$BASE/graphql" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    -d '{"operationName":"OrderDetail","variables":{"id":"ord_8821"},"query":"query OrderDetail($id: ID!) {\n  order(id: $id) {\n    id\n    total\n    customer { id name email }\n    items { sku name qty unitPrice }\n  }\n}"}'
}

r_twilio_sms() {
  fire "POST   /api/sms/send (Twilio form)" -X POST "$BASE/api/sms/send" \
    -H "Content-Type: application/x-www-form-urlencoded" \
    -H "Authorization: Basic QUNmYWtlOnNlY3JldA==" \
    --data-urlencode "From=+14155550100" \
    --data-urlencode "To=+14155550199" \
    --data-urlencode "Body=Your verification code is 482910. It expires in 10 minutes."
}

r_analytics_event() {
  fire "POST   /v1/events/track" -X POST "$BASE/v1/events/track" \
    -H "Content-Type: application/json" \
    -H "X-Api-Key: pk_live_fake_abcdef" \
    -d '{"event":"checkout_completed","timestamp":"'"$(date -u +%FT%TZ)"'","user":{"id":"u_42","email":"matt@matthorner.co.uk","plan":"pro"},"properties":{"order_id":"ord_8821","total":49.99,"currency":"USD","items":2,"discount_code":"BACK2SCHOOL"},"context":{"ip":"203.0.113.42","ua":"Mozilla/5.0","page":{"path":"/checkout","referrer":"https://google.com"}}}'
}

r_photo_upload_raw() {
  fire "PUT    /assets/photo.jpg (raw bytes)" -X PUT "$BASE/assets/photo.jpg" \
    -H "Content-Type: image/jpeg" \
    -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.fake.payload" \
    --data-binary "@$TMP/photo.jpg"
}

r_sendgrid_batch() {
  fire "POST   /webhooks/sendgrid (event batch)" -X POST "$BASE/webhooks/sendgrid" \
    -H "Content-Type: application/json" \
    -H "X-Twilio-Email-Event-Webhook-Signature: fake-sig" \
    -d '[{"email":"a@example.com","event":"delivered","sg_event_id":"e1","sg_message_id":"m1","timestamp":'"$(date +%s)"'},{"email":"b@example.com","event":"open","useragent":"Mozilla/5.0","ip":"203.0.113.7","sg_event_id":"e2","sg_message_id":"m2","timestamp":'"$(date +%s)"'},{"email":"c@example.com","event":"bounce","reason":"550 mailbox full","sg_event_id":"e3","sg_message_id":"m3","timestamp":'"$(date +%s)"'}]'
}

# ----- Drive runners until COUNT requests have fired -----
total="${#RUNNERS[@]}"
for ((i=0; i<COUNT; i++)); do
  "${RUNNERS[$((i % total))]}"
done

echo "----"
echo "Done — $COUNT requests fired at $BASE"
