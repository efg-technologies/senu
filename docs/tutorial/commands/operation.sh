# ANCHOR: launch
curl -sS -w '\nHTTP %{http_code}\n' -H 'Authorization: Bearer demo-api-token' \
  'http://127.0.0.1:3000/api/projects?q=launch'
# ANCHOR_END: launch
# ANCHOR: other-tenant
curl -sS -w '\nHTTP %{http_code}\n' -H 'Authorization: Bearer demo-api-token' \
  'http://127.0.0.1:3000/api/projects?q=globex'
# ANCHOR_END: other-tenant
# ANCHOR: pick-tenant
curl -sS -w '\nHTTP %{http_code}\n' -H 'Authorization: Bearer demo-api-token' \
  'http://127.0.0.1:3000/api/projects?tenant=globex'
# ANCHOR_END: pick-tenant
# ANCHOR: bad-search
curl -sS -w '\nHTTP %{http_code}\n' -H 'Authorization: Bearer demo-api-token' \
  'http://127.0.0.1:3000/api/projects?q=a%07b'
# ANCHOR_END: bad-search
# ANCHOR: no-token
curl -sS -w '\nHTTP %{http_code}\n' 'http://127.0.0.1:3000/api/projects'
# ANCHOR_END: no-token
