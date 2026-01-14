use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

pub struct RiskClassifier;

impl RiskClassifier {
    /// Classify risk based on file path
    pub fn classify_path(path: &str) -> RiskLevel {
        let path = Path::new(path);
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let path_str = path.to_str().unwrap_or("");

        // Critical: Boot, kernel, and core system files
        if path_str.contains("boot")
            || path_str.contains("hardware-configuration")
            || filename == "flake.lock"
            || path_str.contains("/etc/nixos")
        {
            return RiskLevel::Critical;
        }

        // High: System modules and services
        if path_str.contains("modules/") && !path_str.contains("home/")
            || path_str.contains("system.nix")
            || path_str.contains("networking")
            || path_str.contains("firewall")
        {
            return RiskLevel::High;
        }

        // Medium: User configs and home-manager
        if path_str.contains("home/")
            || path_str.contains("users/")
            || path_str.ends_with(".nix")
        {
            return RiskLevel::Medium;
        }

        // Low: Everything else
        RiskLevel::Low
    }

    /// Classify risk based on diff content
    pub fn classify_diff(diff: &str) -> RiskLevel {
        let mut risk = RiskLevel::Low;

        // Check for dangerous patterns
        let critical_patterns = [
            "boot.loader",
            "fileSystems",
            "swapDevices",
            "networking.firewall.enable = false",
            "security.sudo.wheelNeedsPassword = false",
            "PermitRootLogin yes",
        ];

        let high_patterns = [
            "services.openssh",
            "networking.firewall",
            "security.",
            "systemd.services",
            "users.users.root",
        ];

        let medium_patterns = [
            "environment.systemPackages",
            "programs.",
            "services.",
        ];

        for pattern in critical_patterns {
            if diff.contains(pattern) {
                return RiskLevel::Critical;
            }
        }

        for pattern in high_patterns {
            if diff.contains(pattern) {
                risk = RiskLevel::High;
            }
        }

        if risk == RiskLevel::Low {
            for pattern in medium_patterns {
                if diff.contains(pattern) {
                    risk = RiskLevel::Medium;
                }
            }
        }

        risk
    }

    /// Combine path and content risk (take higher)
    pub fn classify(path: &str, diff: &str) -> RiskLevel {
        let path_risk = Self::classify_path(path);
        let diff_risk = Self::classify_diff(diff);

        if path_risk as u8 > diff_risk as u8 {
            path_risk
        } else {
            diff_risk
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_paths() {
        assert_eq!(RiskClassifier::classify_path("hosts/foo/hardware-configuration.nix"), RiskLevel::Critical);
        assert_eq!(RiskClassifier::classify_path("flake.lock"), RiskLevel::Critical);
    }

    #[test]
    fn test_high_paths() {
        assert_eq!(RiskClassifier::classify_path("modules/system.nix"), RiskLevel::High);
        assert_eq!(RiskClassifier::classify_path("modules/networking.nix"), RiskLevel::High);
    }

    #[test]
    fn test_medium_paths() {
        assert_eq!(RiskClassifier::classify_path("home/modules/git.nix"), RiskLevel::Medium);
        assert_eq!(RiskClassifier::classify_path("users/hashwarlock/home.nix"), RiskLevel::Medium);
    }

    #[test]
    fn test_low_paths() {
        assert_eq!(RiskClassifier::classify_path("README.md"), RiskLevel::Low);
        assert_eq!(RiskClassifier::classify_path("docs/notes.txt"), RiskLevel::Low);
    }

    #[test]
    fn test_critical_diff() {
        let diff = "+  boot.loader.grub.device = \"/dev/sda\";";
        assert_eq!(RiskClassifier::classify_diff(diff), RiskLevel::Critical);
    }
}
