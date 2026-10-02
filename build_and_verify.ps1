# Script de Build e Verificação do DuckDNS Updater (Windows)
param (
    [string]$Notes = "Atualização automática e melhorias de estabilidade"
)

$ErrorActionPreference = "Stop"

# 1. Carrega versão do Cargo.toml
$cargoToml = Get-Content "Cargo.toml" -Raw
if ($cargoToml -match 'version\s*=\s*"([^"]+)"') {
    $version = $matches[1]
    Write-Host "[*] Versão detectada no Cargo.toml: v$version" -ForegroundColor Cyan
} else {
    Write-Error "Não foi possível detectar a versão no Cargo.toml"
    exit 1
}

# 2. Compilação em modo release
Write-Host "[*] Compilando duckdns-updater v$version em modo release..." -ForegroundColor Yellow
cargo build --release

$outputBinary = "target\release\duckdns-updater.exe"
if (-not (Test-Path $outputBinary)) {
    Write-Error "Binário não encontrado em $outputBinary"
    exit 1
}

# 3. Garante que o Resource Directory (IMAGE_DIRECTORY_ENTRY_RESOURCE) do PE aponte para .rsrc
Write-Host "[*] Verificando e garantindo tabela de recursos (.rsrc) no cabeçalho PE..." -ForegroundColor Cyan
python -c "
import struct
path = r'$outputBinary'
with open(path, 'rb') as f:
    data = bytearray(f.read())

pe_offset = struct.unpack_from('<I', data, 0x3c)[0]
opt_offset = pe_offset + 24
magic = struct.unpack_from('<H', data, opt_offset)[0]
data_dir_base = opt_offset + (112 if magic == 0x20b else 96)
rsrc_dir_offset = data_dir_base + 16*2

num_sections = struct.unpack_from('<H', data, pe_offset + 6)[0]
sec_size = struct.unpack_from('<H', data, pe_offset + 20)[0]
sec_headers_offset = opt_offset + sec_size

rsrc_va = 0
rsrc_raw_sz = 0
for i in range(num_sections):
    sec = data[sec_headers_offset + i*40 : sec_headers_offset + (i+1)*40]
    name = sec[:8].rstrip(b'\x00').decode('ascii', errors='ignore')
    if name == '.rsrc':
        vsize, vaddr, raw_size, raw_offset = struct.unpack_from('<IIII', sec, 8)
        rsrc_va = vaddr
        rsrc_raw_sz = raw_size
        break

if rsrc_va > 0:
    curr_rva, curr_sz = struct.unpack_from('<II', data, rsrc_dir_offset)
    if curr_rva == 0:
        struct.pack_into('<II', data, rsrc_dir_offset, rsrc_va, rsrc_raw_sz)
        with open(path, 'wb') as f:
            f.write(data)
        print('    [OK] Cabecalho PE atualizado: ponteiro .rsrc configurado!')
    else:
        print('    [OK] Ponteiro .rsrc ja configurado corretamente.')
"

# 4. Notifica o Shell do Windows para atualizar o cache de ícones
python -c "
import ctypes
shell32 = ctypes.windll.shell32
user32 = ctypes.windll.user32
path = r'$outputBinary'
hicon = shell32.ExtractIconW(0, path, 0)
if hicon > 1:
    print('    [OK] Icone nativo verificado com sucesso pelo Windows Shell!')
    user32.DestroyIcon(hicon)
shell32.SHChangeNotify(0x08000000, 0, None, None)
"

# 5. Sincroniza executável para a raiz e pasta dist (como solicitado)
Write-Host "[*] Sincronizando executável na raiz e em dist/..." -ForegroundColor Cyan
if (-not (Test-Path "dist")) { New-Item -ItemType Directory -Path "dist" | Out-Null }
Copy-Item -Force $outputBinary "dist\duckdns-updater.exe"
Copy-Item -Force $outputBinary "duckdns-updater.exe"

# 6. Notifica o Shell para a raiz também
python -c "
import ctypes
shell32 = ctypes.windll.shell32
user32 = ctypes.windll.user32
path = r'duckdns-updater.exe'
hicon = shell32.ExtractIconW(0, path, 0)
if hicon > 1:
    user32.DestroyIcon(hicon)
shell32.SHChangeNotify(0x08000000, 0, None, None)
"

Write-Host "`n[SUCESSO] Build da versão v$version concluído com sucesso!" -ForegroundColor Green
Write-Host "Executável pronto em: duckdns-updater.exe (raiz) e $outputBinary" -ForegroundColor Green

