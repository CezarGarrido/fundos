# Definir variáveis
$ZIP_URL = "https://github.com/CezarGarrido/fundos/releases/download/main/fundos-x86_64-pc-windows-msvc.zip"
$ZIP_FILE = "fundos.zip"
$EXTRACT_DIR = "fundos"
$INNER_DIR = "fundos-x86_64-pc-windows-msvc" # Nome da pasta interna do zip

# Baixar o arquivo zipado
Write-Output "Baixando o programa..."
Invoke-WebRequest -Uri $ZIP_URL -OutFile $ZIP_FILE

# Extrair o zip e remover o arquivo
Write-Output "Extraindo e limpando..."
Expand-Archive -Path $ZIP_FILE -DestinationPath $EXTRACT_DIR
Remove-Item $ZIP_FILE

# Mover os arquivos internos para o diretório atual e remover a pasta intermediária
Move-Item "$EXTRACT_DIR\$INNER_DIR\*" . -Force
Remove-Item $EXTRACT_DIR -Recurse

# Executar o programa
Write-Output "Iniciando o programa..."
Start-Process "fundos-x86_64-pc-windows-msvc.exe"

