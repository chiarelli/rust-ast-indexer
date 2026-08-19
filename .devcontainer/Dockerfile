# ============================================================
# STAGE 1: builder — toolchain Rust + ferramentas compiladas
# ============================================================
FROM rust:1.97-slim AS rust-builder

# rustfmt + clippy não vêm no perfil slim (download pequeno)
RUN rustup component add rustfmt clippy

# cargo-show-asm não tem binário pré-compilado; compilar aqui
# (o registry fica populado e é copiado para a imagem final)
RUN cargo install cargo-show-asm

# ============================================================
# STAGE 2: final — Ubuntu 24.04 (glibc 2.39, necessário p/ irust)
# ============================================================
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Base + ferramentas + Node.js repo
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    nano \
    tree \
    file \
    curl \
    wget \
    ca-certificates \
    pkg-config \
    libssl-dev \
    unzip \
    zip \
    zsh \
    gdb \
    lldb \
    gnupg \
    python3 \
    make g++ \
    trash-cli \
    ssh \
    && rm -rf /var/lib/apt/lists/*

# -----------------------------
# Node.js
# -----------------------------
ARG NODE_VERSION=22.19.0

RUN curl -fsSL https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz \
    -o node.tar.xz \
    && tar -xJf node.tar.xz -C /usr/local --strip-components=1 \
    && rm node.tar.xz

RUN node --version \
    && npm --version

RUN corepack enable \
    && corepack prepare pnpm@latest --activate

# Usuário não-root
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID

# Associando usúario ao grupo
# Ubuntu 24.04 cria usuário 'ubuntu' com UID/GID 1000; remover para liberar o ID
RUN userdel -r ubuntu 2>/dev/null || true \
    && groupdel ubuntu 2>/dev/null || true \
    && groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && chsh -s /usr/bin/zsh $USERNAME \
    && mkdir -p /workspace \
    && chown -R $USERNAME:$USERNAME /workspace

RUN ln -s /usr/bin/clear /usr/bin/cls \
    # Instalando o Crush AI DevTools
    && npm install -g @charmland/crush

# Toolchain Rust copiada do builder (evita download de ~4 min do rustup)
COPY --from=rust-builder /usr/local/rustup /home/${USERNAME}/.rustup
COPY --from=rust-builder /usr/local/cargo/bin /home/${USERNAME}/.cargo/bin
COPY --from=rust-builder /usr/local/cargo/registry /home/${USERNAME}/.cargo/registry

RUN chown -R ${USERNAME}:${USERNAME} /home/${USERNAME}/.rustup /home/${USERNAME}/.cargo

ENV RUSTUP_HOME="/home/${USERNAME}/.rustup" \
    CARGO_HOME="/home/${USERNAME}/.cargo" \
    PATH="/home/${USERNAME}/.cargo/bin:${PATH}"

USER $USERNAME
WORKDIR /workspace

# Ferramentas cargo com binário pré-compilado
# (compilar via cargo install leva 5-15 min por ferramenta)
RUN mkdir -p /tmp/rust-tools \
    && curl -fsSL https://github.com/watchexec/cargo-watch/releases/download/v8.5.3/cargo-watch-v8.5.3-x86_64-unknown-linux-gnu.tar.xz \
        -o /tmp/rust-tools/cargo-watch.tar.xz \
    && tar -xJf /tmp/rust-tools/cargo-watch.tar.xz -C /tmp/rust-tools \
    && mv /tmp/rust-tools/cargo-watch-v8.5.3-x86_64-unknown-linux-gnu/cargo-watch \
       /home/${USERNAME}/.cargo/bin/cargo-watch \
    && curl -fsSL https://github.com/sigmaSd/IRust/releases/download/irust%401.77.1/irust-x86_64-unknown-linux-gnu \
        -o /home/${USERNAME}/.cargo/bin/irust \
    && chmod +x /home/${USERNAME}/.cargo/bin/irust \
    && rm -rf /tmp/rust-tools

# Oh My Zsh
RUN sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended \
    && git clone --depth=1 https://github.com/zsh-users/zsh-autosuggestions /tmp/zsh-autosuggestions \
    && mkdir -p /home/${USERNAME}/.oh-my-zsh/custom/plugins \
    && mv /tmp/zsh-autosuggestions \
       /home/${USERNAME}/.oh-my-zsh/custom/plugins/zsh-autosuggestions \
    && rm -rf /tmp/zsh-autosuggestions \
    && sed -i 's/^ZSH_THEME=.*/ZSH_THEME="robbyrussell"/' /home/${USERNAME}/.zshrc \
    && sed -i 's/^plugins=.*/plugins=(git zsh-autosuggestions)/' /home/${USERNAME}/.zshrc \
    && echo '\n# Histórico melhor' >> /home/${USERNAME}/.zshrc \
    && echo 'HISTSIZE=10000' >> /home/${USERNAME}/.zshrc \
    && echo 'SAVEHIST=10000' >> /home/${USERNAME}/.zshrc \
    && echo 'setopt share_history' >> /home/${USERNAME}/.zshrc

RUN mkdir -p /home/${USERNAME}/.local/share/kilo