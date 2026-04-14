FROM ubuntu:22.04

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

# Node.js 18 (oficial)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get update && apt-get install -y --no-install-recommends \
    nodejs \
    && npm install -g npm@latest \
    && rm -rf /var/lib/apt/lists/*

# Usuário não-root
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=$USER_UID

# Associando usúario ao grupo
RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USER_GID -m $USERNAME \
    && chsh -s /usr/bin/zsh $USERNAME \
    && mkdir -p /workspace \
    && chown -R $USERNAME:$USERNAME /workspace
    
RUN ln -s /usr/bin/clear /usr/bin/cls \
    # Instalando o Crush AI DevTools
    && npm install -g @charmland/crush
    
USER $USERNAME
WORKDIR /workspace

# Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/home/${USERNAME}/.cargo/bin:${PATH}"

RUN rustup install nightly \
    && cargo install cargo-watch \
    && cargo install irust cargo-show-asm cargo-expand \
    && rustup component add rustfmt clippy

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