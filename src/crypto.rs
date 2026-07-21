//! 加密模块：AES-256-GCM + Argon2id
//! 
//! 安全设计：
//! - 密钥派生：Argon2id（抗 GPU/ASIC，64MB 内存成本）
//! - 加密算法：AES-256-GCM（认证加密，防篡改）
//! - 每个文件使用独立随机 Salt（16 字节）和 Nonce（12 字节）

use aes_gcm::{Aes256Gcm, KeyInit, aead::{Aead, OsRng}};
use aes_gcm::aead::rand_core::RngCore;
use argon2::{Argon2, password_hash::SaltString};
use serde::{Serialize, Deserialize};

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// 加密后的数据
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedData {
    /// Base64 编码的盐值
    pub salt: String,
    /// Base64 编码的 nonce
    pub nonce: String,
    /// Base64 编码的密文
    pub ciphertext: String,
}

/// 从密码派生 256-bit 密钥（Argon2id）
pub fn derive_key(password: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(64 * 1024, 3, 4, Some(32))
            .expect("Argon2 参数错误"),
    );
    argon2.hash_password_into(password.as_bytes(), salt, &mut key)
        .expect("密钥派生失败");
    key
}

/// 生成随机字节
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    RngCore::fill_bytes(&mut OsRng, &mut buf);
    buf
}

/// 加密数据
pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> EncryptedData {
    let salt = random_bytes::<SALT_LEN>();
    let nonce_bytes = random_bytes::<NONCE_LEN>();
    
    let cipher = Aes256Gcm::new_from_slice(key).expect("密钥长度错误");
    let ciphertext = cipher.encrypt(&nonce_bytes.into(), plaintext)
        .expect("加密失败");
    
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    EncryptedData {
        salt: STANDARD.encode(&salt),
        nonce: STANDARD.encode(&nonce_bytes),
        ciphertext: STANDARD.encode(&ciphertext),
    }
}

/// 解密数据
pub fn decrypt(data: &EncryptedData, key: &[u8; 32]) -> Result<Vec<u8>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    
    let nonce = STANDARD.decode(&data.nonce).map_err(|e| format!("Base64 解码失败: {}", e))?;
    let ciphertext = STANDARD.decode(&data.ciphertext).map_err(|e| format!("Base64 解码失败: {}", e))?;
    
    let cipher = Aes256Gcm::new_from_slice(key).expect("密钥长度错误");
    cipher.decrypt(nonce.as_slice().into(), ciphertext.as_ref())
        .map_err(|_| "解密失败（密码错误或数据损坏）".to_string())
}

/// 生成恢复密钥（32 字符，含分隔符）
pub fn generate_recovery_key() -> String {
    let bytes = random_bytes::<24>();
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let encoded = STANDARD.encode(&bytes);
    // 格式：XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
    let mut result = String::new();
    for (i, c) in encoded.chars().take(24).enumerate() {
        if i > 0 && i % 4 == 0 {
            result.push('-');
        }
        result.push(c);
    }
    result
}

/// 从恢复密钥派生密钥
pub fn recovery_key_to_key(recovery_key: &str, salt: &[u8]) -> [u8; 32] {
    let cleaned: String = recovery_key.chars().filter(|c| *c != '-').collect();
    derive_key(&cleaned, salt)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt() {
        let password = "TestPassword123!";
        let salt = random_bytes::<SALT_LEN>();
        let key = derive_key(password, &salt);
        
        let plaintext = b"Hello, World!";
        let encrypted = encrypt(plaintext, &key);
        let decrypted = decrypt(&encrypted, &key).unwrap();
        
        assert_eq!(plaintext.to_vec(), decrypted);
    }
    
    #[test]
    fn test_wrong_password_fails() {
        let password = "TestPassword123!";
        let wrong_password = "WrongPassword!";
        let salt = random_bytes::<SALT_LEN>();
        
        let key = derive_key(password, &salt);
        let wrong_key = derive_key(wrong_password, &salt);
        
        let plaintext = b"Hello, World!";
        let encrypted = encrypt(plaintext, &key);
        
        assert!(decrypt(&encrypted, &wrong_key).is_err());
    }
}
