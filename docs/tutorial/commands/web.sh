# ANCHOR: html
curl -sS -u demo:demo-password 'http://127.0.0.1:3000/projects?q=notes' | grep 'data-project-id'
# ANCHOR_END: html
# ANCHOR: json
curl -sS -w '\n' -H 'Authorization: Bearer demo-api-token' \
  'http://127.0.0.1:3000/api/projects?q=notes'
# ANCHOR_END: json
# ANCHOR: echoed-search
curl -sS -u demo:demo-password 'http://127.0.0.1:3000/projects?q=%3Cb%3E' | grep 'value='
# ANCHOR_END: echoed-search
# ANCHOR: wrong-credential
curl -sS -o /dev/null -w '%{http_code} %header{www-authenticate}\n' \
  -H 'Authorization: Bearer demo-api-token' 'http://127.0.0.1:3000/projects'
# ANCHOR_END: wrong-credential
