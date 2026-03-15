//! 技能签名验证系统
//!
//! 提供技能完整性验证和来源认证，防止供应链攻击。

use anyhow::Result;
use tracing::info;
use std::path::Path;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::fs;

/// 签名验证结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerificationResult {
    /// 已验证且可信
    Trusted {
        fingerprint: String,
        signer: String,
        timestamp: i64,
    },
    /// 已验证但不在信任列表中
    Untrusted {
        fingerprint: String,
    },
    /// 未签名
    Unsigned,
    /// 签名无效
    Invalid {
        reason: String,
    },
    /// 验证过程中出错
    Error {
        message: String,
    },
}

impl VerificationResult {
    /// 检查是否允许安装
    pub fn is_allowed(&self, allow_unsigned: bool) -> bool {
        match self {
            VerificationResult::Trusted { .. } => true,
            VerificationResult::Unsigned => allow_unsigned,
            _ => false,
        }
    }

    /// 获取验证状态描述
    pub fn status(&self) -> &'static str {
        match self {
            VerificationResult::Trusted { .. } => "trusted",
            VerificationResult::Untrusted { .. } => "untrusted",
            VerificationResult::Unsigned => "unsigned",
            VerificationResult::Invalid { .. } => "invalid",
            VerificationResult::Error { .. } => "error",
        }
    }
}

/// 公钥
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub fingerprint: String,
    pub owner: String,
    pub key_data: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

impl PublicKey {
    /// 验证签名
    pub fn verify(&self, data: &[u8], signature: &str) -> Result<bool> {
        // 这里使用简单的哈希验证作为示例
        // 实际生产环境应该使用 ed25519 或 RSA
        let computed_hash = Self::compute_hash(data);
        let signature_hash = Self::decode_signature(signature)?;
        
        Ok(computed_hash == signature_hash)
    }

    /// 计算数据哈希（简化版）
    fn compute_hash(data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// 解码签名
    fn decode_signature(signature: &str) -> Result<String> {
        // 简化实现，实际应该使用 base64 解码
        Ok(signature.to_string())
    }
}

/// 技能签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSignature {
    pub version: String,
    pub fingerprint: String,
    pub timestamp: i64,
    pub manifest_hash: String,
    pub signature: String,
}

/// 签名验证器
pub struct SkillSignatureVerifier {
    trusted_keys: HashMap<String, PublicKey>,
    allow_expired_keys: bool,
}

impl SkillSignatureVerifier {
    /// 创建新的验证器
    pub fn new() -> Self {
        Self {
            trusted_keys: HashMap::new(),
            allow_expired_keys: false,
        }
    }

    /// 从配置加载可信密钥
    pub fn with_config(config: &SignatureConfig) -> Self {
        let mut verifier = Self::new();
        
        for key in &config.trusted_keys {
            verifier.add_trusted_key(key.clone());
        }
        
        verifier.allow_expired_keys = config.allow_expired_keys;
        verifier
    }

    /// 添加可信密钥
    pub fn add_trusted_key(&mut self, key: PublicKey) {
        info!("Adding trusted key: {} ({})", key.fingerprint, key.owner);
        self.trusted_keys.insert(key.fingerprint.clone(), key);
    }

    /// 移除可信密钥
    pub fn remove_trusted_key(&mut self, fingerprint: &str) {
        self.trusted_keys.remove(fingerprint);
    }

    /// 验证技能签名
    pub async fn verify(&self, skill_dir: &Path) -> VerificationResult {
        // 1. 查找签名文件
        let sig_file = skill_dir.join(".skill.sig");
        let manifest_file = skill_dir.join("skill.yaml");
        
        if !sig_file.exists() {
            // 检查是否有其他格式的签名
            let alt_sig_files = [
                skill_dir.join("signature.json"),
                skill_dir.join(".signature"),
            ];
            
            let found_alt = alt_sig_files.iter().find(|p| p.exists());
            if found_alt.is_none() {
                return VerificationResult::Unsigned;
            }
        }

        // 2. 读取签名
        let signature_content = match fs::read_to_string(&sig_file).await {
            Ok(content) => content,
            Err(e) => {
                return VerificationResult::Error {
                    message: format!("Failed to read signature file: {}", e),
                };
            }
        };

        let signature: SkillSignature = match serde_json::from_str(&signature_content) {
            Ok(sig) => sig,
            Err(e) => {
                return VerificationResult::Invalid {
                    reason: format!("Failed to parse signature: {}", e),
                };
            }
        };

        // 3. 读取 manifest
        let manifest_content = match fs::read_to_string(&manifest_file).await {
            Ok(content) => content,
            Err(e) => {
                return VerificationResult::Error {
                    message: format!("Failed to read manifest: {}", e),
                };
            }
        };

        // 4. 验证 manifest 哈希
        let computed_hash = Self::compute_manifest_hash(&manifest_content);
        if computed_hash != signature.manifest_hash {
            return VerificationResult::Invalid {
                reason: "Manifest hash mismatch".to_string(),
            };
        }

        // 5. 查找公钥并验证
        if let Some(public_key) = self.trusted_keys.get(&signature.fingerprint) {
            // 检查密钥是否过期
            if !self.allow_expired_keys {
                if let Some(expires_at) = public_key.expires_at {
                    let now = chrono::Utc::now().timestamp();
                    if now > expires_at {
                        return VerificationResult::Invalid {
                            reason: "Signing key has expired".to_string(),
                        };
                    }
                }
            }

            // 验证签名
            match public_key.verify(manifest_content.as_bytes(), &signature.signature) {
                Ok(true) => VerificationResult::Trusted {
                    fingerprint: signature.fingerprint,
                    signer: public_key.owner.clone(),
                    timestamp: signature.timestamp,
                },
                Ok(false) => VerificationResult::Invalid {
                    reason: "Signature verification failed".to_string(),
                },
                Err(e) => VerificationResult::Error {
                    message: format!("Verification error: {}", e),
                },
            }
        } else {
            // 签名有效但密钥不在信任列表中
            VerificationResult::Untrusted {
                fingerprint: signature.fingerprint,
            }
        }
    }

    /// 计算 manifest 哈希
    fn compute_manifest_hash(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// 生成签名（用于技能发布者）
    pub fn sign_manifest(
        manifest_content: &str,
        _private_key: &str,
        fingerprint: &str,
    ) -> Result<SkillSignature> {
        let manifest_hash = Self::compute_manifest_hash(manifest_content);
        let timestamp = chrono::Utc::now().timestamp();
        
        // 这里简化实现，实际应该使用加密签名
        let signature = format!("{}:{}", fingerprint, manifest_hash);
        
        Ok(SkillSignature {
            version: "1.0".to_string(),
            fingerprint: fingerprint.to_string(),
            timestamp,
            manifest_hash,
            signature,
        })
    }
}

impl Default for SkillSignatureVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// 签名配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConfig {
    pub trusted_keys: Vec<PublicKey>,
    pub allow_unsigned: bool,
    pub allow_expired_keys: bool,
    pub require_signature: bool,
}

impl Default for SignatureConfig {
    fn default() -> Self {
        Self {
            trusted_keys: Vec::new(),
            allow_unsigned: true, // 开发阶段允许未签名
            allow_expired_keys: false,
            require_signature: false,
        }
    }
}

/// 签名工具
pub struct SignatureTool;

impl SignatureTool {
    /// 为技能生成签名
    pub async fn sign_skill(
        skill_dir: &Path,
        private_key: &str,
        fingerprint: &str,
    ) -> Result<()> {
        let manifest_path = skill_dir.join("skill.yaml");
        let manifest_content = fs::read_to_string(&manifest_path).await?;
        
        let signature = SkillSignatureVerifier::sign_manifest(
            &manifest_content,
            private_key,
            fingerprint,
        )?;
        
        let sig_content = serde_json::to_string_pretty(&signature)?;
        fs::write(skill_dir.join(".skill.sig"), sig_content).await?;
        
        info!("Generated signature for skill at {:?}", skill_dir);
        Ok(())
    }

    /// 验证技能签名（CLI 工具）
    pub async fn verify_skill(skill_dir: &Path, trusted_keys_path: Option<&Path>) -> Result<VerificationResult> {
        let mut verifier = SkillSignatureVerifier::new();
        
        // 加载可信密钥
        if let Some(keys_path) = trusted_keys_path {
            let keys_content = fs::read_to_string(keys_path).await?;
            let keys: Vec<PublicKey> = serde_json::from_str(&keys_content)?;
            for key in keys {
                verifier.add_trusted_key(key);
            }
        }
        
        Ok(verifier.verify(skill_dir).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_result_allowed() {
        let trusted = VerificationResult::Trusted {
            fingerprint: "abc123".to_string(),
            signer: "Test".to_string(),
            timestamp: 1234567890,
        };
        assert!(trusted.is_allowed(false));
        assert!(trusted.is_allowed(true));

        let unsigned = VerificationResult::Unsigned;
        assert!(!unsigned.is_allowed(false));
        assert!(unsigned.is_allowed(true));

        let invalid = VerificationResult::Invalid {
            reason: "test".to_string(),
        };
        assert!(!invalid.is_allowed(false));
        assert!(!invalid.is_allowed(true));
    }
}
