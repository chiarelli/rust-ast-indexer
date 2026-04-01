#!/bin/bash
set -e

IMAGE_NAME="vsc-crush:1.0"

echo "🔍 Checando imagem: $IMAGE_NAME"

# if [[ -z "$(docker images -q $IMAGE_NAME 2> /dev/null)" ]]; then
#   echo "🚧 Imagem não encontrada. Buildando..."
docker images -q $IMAGE_NAME 2> /dev/null
  # KVM_GID: usa env ou detecta
  if [[ -z "$KVM_GID" ]]; then
    if getent group kvm > /dev/null 2>&1; then
      KVM_GID=$(getent group kvm | cut -d: -f3)
    else
      echo "⚠️ Grupo kvm não encontrado, usando fallback 1000"
      KVM_GID=1000
    fi
  # fi

  # UID/GID: usa env ou detecta
  USER_UID=${USER_UID:-$(id -u)}
  USER_GID=${USER_GID:-$(id -g)}

  echo "➡️ USER_UID=$USER_UID"
  echo "➡️ USER_GID=$USER_GID"

  cd ./.devcontainer
  docker build \
    -t $IMAGE_NAME \
    --build-arg USER_UID=$USER_UID \
    --build-arg USER_GID=$USER_GID \
    .

  echo "✅ Build concluído"
else
  echo "✅ Imagem já existe. Pulando build."
fi