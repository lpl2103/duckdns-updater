//! # Módulo de Proteção Criptográfica DPAPI (`core/crypto.rs`)
//!
//! Protege tokens e credenciais sensíveis em disco utilizando a Data Protection API (DPAPI)
//! nativa do Windows (`crypt32.dll`), vinculada à conta do usuário logado.
//!
//! Se o arquivo de configuração for copiado para outro computador ou conta,
//! os dados não poderão ser descriptografados.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

pub const DPAPI_PREFIX: &str = "dpapi:";

#[cfg(target_os = "windows")]
mod win32 {
    #[repr(C)]
    #[allow(non_snake_case)]
    struct DATA_BLOB {
        cbData: u32,
        pbData: *mut u8,
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            pDataIn: *const DATA_BLOB,
            szDataDescr: *const u16,
            pOptionalEntropy: *const DATA_BLOB,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *mut std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DATA_BLOB,
        ) -> i32;

        fn CryptUnprotectData(
            pDataIn: *const DATA_BLOB,
            ppszDataDescr: *mut *mut u16,
            pOptionalEntropy: *const DATA_BLOB,
            pvReserved: *mut std::ffi::c_void,
            pPromptStruct: *mut std::ffi::c_void,
            dwFlags: u32,
            pDataOut: *mut DATA_BLOB,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    /// RAII Guard para garantir que a memória alocada pela DPAPI seja sempre liberada via LocalFree.
    struct LocalAllocGuard(*mut u8);

    impl Drop for LocalAllocGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    LocalFree(self.0 as *mut _);
                }
            }
        }
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        unsafe {
            let data_in = DATA_BLOB {
                cbData: data.len() as u32,
                pbData: data.as_ptr() as *mut u8,
            };
            let mut data_out = DATA_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            let success = CryptProtectData(
                &data_in,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            );

            if success == 0 || data_out.pbData.is_null() {
                return Err("Falha ao criptografar dados com DPAPI".to_string());
            }

            let _guard = LocalAllocGuard(data_out.pbData);
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            Ok(slice.to_vec())
        }
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        unsafe {
            let data_in = DATA_BLOB {
                cbData: data.len() as u32,
                pbData: data.as_ptr() as *mut u8,
            };
            let mut data_out = DATA_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };

            let success = CryptUnprotectData(
                &data_in,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            );

            if success == 0 || data_out.pbData.is_null() {
                return Err("Falha ao descriptografar dados com DPAPI".to_string());
            }

            let _guard = LocalAllocGuard(data_out.pbData);
            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            Ok(slice.to_vec())
        }
    }
}

/// Criptografa uma string usando DPAPI e retorna com prefixo "dpapi:<base64>".
/// Se o texto for vazio, retorna vazio.
pub fn encrypt_string(plain: &str) -> String {
    if plain.trim().is_empty() {
        return String::new();
    }

    #[cfg(target_os = "windows")]
    {
        match win32::protect(plain.as_bytes()) {
            Ok(encrypted) => format!("{}{}", DPAPI_PREFIX, BASE64.encode(encrypted)),
            Err(_) => plain.to_string(), // Fallback em caso improvável de erro
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        plain.to_string()
    }
}

/// Descriptografa uma string prefixada com "dpapi:<base64>".
/// Se não tiver o prefixo, retorna o valor original (compatibilidade com texto puro).
pub fn decrypt_string(cipher: &str) -> String {
    if let Some(encoded) = cipher.strip_prefix(DPAPI_PREFIX) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(bytes) = BASE64.decode(encoded) {
                if let Ok(decrypted) = win32::unprotect(&bytes) {
                    if let Ok(text) = String::from_utf8(decrypted) {
                        return text;
                    }
                }
            }
        }
    }

    cipher.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_roundtrip() {
        let secret = "a7c4d0c2-c237-4b77-9f12-123456789abc";
        let encrypted = encrypt_string(secret);
        assert!(encrypted.starts_with(DPAPI_PREFIX));
        let decrypted = decrypt_string(&encrypted);
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn test_plain_fallback() {
        let plain = "my-plain-token";
        assert_eq!(decrypt_string(plain), plain);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(encrypt_string(""), "");
        assert_eq!(decrypt_string(""), "");
    }
}
