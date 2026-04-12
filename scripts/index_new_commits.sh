#!/bin/bash

QDRANT_MCP_SERVER_DIR=${QDRANT_MCP_SERVER_DIR:-/workspace/.mcp-tools/qdrant-mcp-server}

cd /workspace && \
cat <<EOF | node --env-file=$QDRANT_MCP_SERVER_DIR/.env $QDRANT_MCP_SERVER_DIR/build/index.js
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"index_new_commits","arguments":{"path":"/workspace"}}}
EOF