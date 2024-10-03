#!/bin/bash

# Definir variáveis
ZIP_URL="https://github.com/CezarGarrido/fundos/releases/download/main/fundos-x86_64-unknown-linux-gnu.zip"
ZIP_FILE="fundos.zip"
EXTRACT_DIR="fundos"
INNER_DIR="fundos-x86_64-unknown-linux-gnu" # Nome da pasta interna do zip

# Baixar o arquivo zipado
echo "Baixando o programa..."
wget -O "$ZIP_FILE" "$ZIP_URL"

# Extrair o zip e remover o arquivo
echo "Extraindo e limpando..."
unzip -q "$ZIP_FILE" -d "$EXTRACT_DIR" && rm "$ZIP_FILE"

# Mover os arquivos internos para o diretório atual e remover a pasta intermediária
mv "$EXTRACT_DIR/$INNER_DIR/"* . && rm -rf "$EXTRACT_DIR"

# Dar permissão de execução ao binário principal
chmod +x fundos-x86_64-unknown-linux-gnu

# Executar o programa
echo "Iniciando o programa..."
./fundos-x86_64-unknown-linux-gnu
