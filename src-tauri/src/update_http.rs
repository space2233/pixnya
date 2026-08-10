use reqwest::{
    redirect::{Attempt, Policy},
    Url,
};
use std::time::Duration;

use crate::updates::is_github_release_url;

pub(crate) const UPDATE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const UPDATE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const UPDATE_USER_AGENT: &str = "PixNya-Updater/1";

pub(crate) fn update_redirect_policy(repository: &str) -> Policy {
    let repository = repository.to_owned();
    Policy::custom(move |attempt| validate_redirect(attempt, &repository))
}

fn validate_redirect(attempt: Attempt<'_>, repository: &str) -> reqwest::redirect::Action {
    if attempt.previous().len() >= 5 {
        return attempt.error("too many update redirects");
    }
    if is_allowed_update_redirect_url(attempt.url(), repository) {
        attempt.follow()
    } else {
        attempt.error("update redirect left the GitHub release origin")
    }
}

fn is_allowed_update_redirect_url(url: &Url, repository: &str) -> bool {
    is_github_release_url(url, repository)
        || (url.scheme() == "https"
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(
                url.host_str(),
                Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
            ))
}

#[cfg(test)]
mod tests {
    use super::is_allowed_update_redirect_url;

    #[test]
    fn update_redirects_stay_with_the_official_repository_or_github_asset_cdn() {
        let official_release = reqwest::Url::parse(
            "https://github.com/space2233/pixnya/releases/download/v1.0.0/latest.json",
        )
        .unwrap();
        let other_repository = reqwest::Url::parse(
            "https://github.com/attacker/pixnya/releases/download/v1.0.0/latest.json",
        )
        .unwrap();
        let github_asset = reqwest::Url::parse(
            "https://release-assets.githubusercontent.com/github-production-release-asset/file?token=signed",
        )
        .unwrap();
        let insecure_asset = reqwest::Url::parse(
            "http://release-assets.githubusercontent.com/github-production-release-asset/file",
        )
        .unwrap();
        let untrusted_asset =
            reqwest::Url::parse("https://downloads.example.invalid/latest.json").unwrap();

        assert!(is_allowed_update_redirect_url(
            &official_release,
            "space2233/pixnya"
        ));
        assert!(!is_allowed_update_redirect_url(
            &other_repository,
            "space2233/pixnya"
        ));
        assert!(is_allowed_update_redirect_url(
            &github_asset,
            "space2233/pixnya"
        ));
        assert!(!is_allowed_update_redirect_url(
            &insecure_asset,
            "space2233/pixnya"
        ));
        assert!(!is_allowed_update_redirect_url(
            &untrusted_asset,
            "space2233/pixnya"
        ));
    }
}
