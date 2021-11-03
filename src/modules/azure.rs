use ini::Ini;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use super::{Context, Module, RootModuleConfig};

type JValue = serde_json::Value;

use crate::configs::azure::AzureConfig;
use crate::formatter::StringFormatter;

type SubscriptionId = String;
type SubscriptionName = String;

pub fn module<'a>(context: &'a Context) -> Option<Module<'a>> {
    let mut module = context.new_module("azure");
    let config = AzureConfig::try_load(module.config);

    if config.disabled {
        //return None;
    };

    let subscription_id = get_azure_subscription_id(context)?;
    let subscription_name = get_azure_subscription_name(context, &subscription_id)?;

    let parsed = StringFormatter::new(config.format).and_then(|formatter| {
        formatter
            .map_meta(|variable, _| match variable {
                "symbol" => Some(config.symbol),
                _ => None,
            })
            .map_style(|variable| match variable {
                "style" => Some(Ok(config.style)),
                _ => None,
            })
            .map(|variable| match variable {
                "subscription" => Some(Ok(subscription_name.to_string())),
                _ => None,
            })
            .parse(None)
    });

    module.set_segments(match parsed {
        Ok(segments) => segments,
        Err(error) => {
            log::warn!("Error in module `azure`:\n{}", error);
            return None;
        }
    });

    Some(module)
}

fn get_config_file_location(context: &Context) -> Option<PathBuf> {
    context
        .get_env("AZURE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let mut home = context.get_home()?;
            home.push(".azure");
            Some(home)
        })
}

fn get_azure_subscription_id(context: &Context) -> Option<SubscriptionId> {
    let mut config_path = get_config_file_location(context)?;    
    config_path.push("clouds.config");

    let config_file = Ini::load_from_file(config_path.as_path()).ok()?;
    
    let azure_cloud_section = config_file.section(Some("AzureCloud")).unwrap();
    let current_subscription_id = azure_cloud_section.get("subscription").unwrap();
    
    return Some(current_subscription_id.to_string());
  }

fn get_azure_subscription_name(context: &Context, subscription_id: &SubscriptionId) -> Option<SubscriptionName> {
    let mut config_path = get_config_file_location(context)?; 
    config_path.push("azureProfile.json");

    if let Some(parsed_json) = parse_json(&config_path) {
        let subscriptions = parsed_json["subscriptions"].as_array()?;

        subscriptions
            .iter()
            .find_map(|s| find_subscription_name(s, subscription_id.to_string()))
    } else {
        return None;
    }
}

fn parse_json(json_file_path: &PathBuf) -> Option<JValue> {
    let mut buffer: Vec<u8> = Vec::new();

    if let Some(json_file) = File::open(&json_file_path).ok() {
        let mut reader = BufReader::new(json_file);
        reader.read_to_end(&mut buffer).ok()?;
    } else {
      return None
    }

    let bytes = buffer.as_mut_slice();
    let decodedbuffer;

    if let Some(&[239, 187, 191]) = bytes.get(0..2) {
        decodedbuffer = bytes.strip_prefix(&[239, 187, 191]).unwrap();
    } else {
        decodedbuffer = bytes;
    }

    let parsed_json: JValue = serde_json::from_slice(&decodedbuffer).ok()?;
    return Some(parsed_json);
}

fn find_subscription_name(subscription: &JValue, current_subscription_id: SubscriptionId) -> Option<SubscriptionName> {
    let subscription_id = subscription["id"].as_str()?;

    if subscription_id == current_subscription_id {
        let subscription_name = subscription["name"].as_str()?;
        return Some(subscription_name.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use crate::test::ModuleRenderer;
    use ansi_term::Color;
    use ini::Ini;
    use std::fs::File;
    use std::io::{self, Write};

    use tempfile::TempDir;

    fn generate_test_config(
        dir: &TempDir,
        cloud_config_contents: &Ini,
        azure_profile_contents: &str,
    ) -> io::Result<()> {
        let clouds_config_path = dir.path().join("clouds.config");
        cloud_config_contents.write_to_file(clouds_config_path.as_path())?;

        let azure_profile_path = dir.path().join("azureProfile.json");
        let mut azure_profile_file = File::create(&azure_profile_path)?;
        azure_profile_file.write_all(azure_profile_contents.as_bytes())?;

        azure_profile_file.sync_all()?;
        Ok(())
    }

    #[test]
    fn subscription_set_correctly() -> io::Result<()> {
        let dir = tempfile::tempdir()?;

        let mut clouds_config_ini = Ini::new();
        clouds_config_ini
            .with_section(Some("AzureCloud"))
            .set("subscription", "f3935dc9-92b5-9a93-da7b-42c325d86939");

        let azure_profile_contents = r#"{
            "installationId": "3deacd2a-b9db-77e1-aa42-23e2f8dfffc3",
            "subscriptions": [
              {
                "id": "f3935dc9-92b5-9a93-da7b-42c325d86939",
                "name": "Subscription 1",
                "state": "Enabled",
                "user": {
                  "name": "user@domain.com",
                  "type": "user"
                },
                "isDefault": true,
                "tenantId": "f0273a19-7779-e40a-00a1-53b8331b3bb6",
                "environmentName": "AzureCloud",
                "homeTenantId": "f0273a19-7779-e40a-00a1-53b8331b3bb6",
                "managedByTenants": []
              },
              {
                "id": "f568c543-d12e-de0b-3d85-69843598b565",
                "name": "Subscription 2",
                "state": "Enabled",
                "user": {
                  "name": "user@domain.com",
                  "type": "user"
                },
                "isDefault": false,
                "tenantId": "0e8a15ec-b0f5-d355-7062-8ece54c59aee",
                "environmentName": "AzureCloud",
                "homeTenantId": "0e8a15ec-b0f5-d355-7062-8ece54c59aee",
                "managedByTenants": []
              },
              {
                "id": "d4442d26-ea6d-46c4-07cb-4f70b8ae5465",
                "name": "Subscription 3",
                "state": "Enabled",
                "user": {
                  "name": "user@domain.com",
                  "type": "user"
                },
                "isDefault": false,
                "tenantId": "a4e1bb4b-5330-2d50-339d-b9674d3a87bc",
                "environmentName": "AzureCloud",
                "homeTenantId": "a4e1bb4b-5330-2d50-339d-b9674d3a87bc",
                "managedByTenants": []
              }
            ]
          }
        "#;

        generate_test_config(&dir, &clouds_config_ini, azure_profile_contents)?;
        let dir_path = &dir.path().to_string_lossy();
        let actual = ModuleRenderer::new("azure")
            .config(toml::toml! {
            [azure]
            disabled = false
            })
            .env("AZURE_CONFIG_DIR", dir_path.as_ref())
            .collect();
        let expected = Some(format!(
            "on {} ",
            Color::Blue.bold().paint("ﴃ Subscription 1")
        ));
        assert_eq!(actual, expected);
        dir.close()
    }

    #[test]
    fn subscription_azure_profile_empty() -> io::Result<()> {
        let dir = tempfile::tempdir()?;

        let mut clouds_config_ini = Ini::new();
        clouds_config_ini
            .with_section(Some("AzureCloud"))
            .set("subscription", "f3935dc9-92b5-9a93-da7b-42c325d86939");

        let azure_profile_contents = r#"{
            "installationId": "3deacd2a-b9db-77e1-aa42-23e2f8dfffc3",
            "subscriptions": []
          }
        "#;

        generate_test_config(&dir, &clouds_config_ini, azure_profile_contents)?;
        let dir_path = &dir.path().to_string_lossy();
        let actual = ModuleRenderer::new("azure")
            .config(toml::toml! {
              [azure]
              disabled = false
            })
            .env("AZURE_CONFIG_DIR", dir_path.as_ref())
            .collect();
        let expected = None;
        assert_eq!(actual, expected);
        dir.close()
    }

    #[test]
    fn files_missing() -> io::Result<()> {
        let dir = tempfile::tempdir()?;

        let dir_path = &dir.path().to_string_lossy();

        let actual = ModuleRenderer::new("azure")
            .env("AZURE_CONFIG_DIR", dir_path.as_ref())
            .collect();
        let expected = None;
        assert_eq!(actual, expected);
        dir.close()
    }
}
