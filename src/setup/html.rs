//! HTML templates for the setup wizard.
//!
//! The setup wizard has three screens:
//! 1. **Setup Form** - Collects Trakt username and optional IDs
//! 2. **OAuth Screen** - Displays device code for Trakt authorization
//! 3. **Success Screen** - Confirms setup completion

// =============================================================================
// Constants
// =============================================================================

const APP_NAME: &str = "Discrakt";
const APP_TAGLINE: &str = "Trakt to Discord Rich Presence";
const GITHUB_URL: &str = "https://github.com/afonsojramos/discrakt";

const TRAKT_SETTINGS_URL: &str = "https://trakt.tv/settings";

const COLOR_SUCCESS: &str = "#4CAF50";

// =============================================================================
// CSS Styles
// =============================================================================

#[allow(clippy::too_many_lines)]
fn styles() -> &'static str {
    r#"
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
            min-height: 100vh;
            display: flex;
            justify-content: center;
            align-items: center;
            padding: 20px;
            color: #e0e0e0;
            -webkit-user-select: none;
            -moz-user-select: none;
            -ms-user-select: none;
            user-select: none;
            cursor: default;
        }

        input[type="text"], input[type="number"], textarea {
            -webkit-user-select: text;
            -moz-user-select: text;
            -ms-user-select: text;
            user-select: text;
            cursor: text;
        }

        .container {
            background: rgba(255, 255, 255, 0.05);
            backdrop-filter: blur(10px);
            border-radius: 16px;
            padding: 40px;
            max-width: 500px;
            width: 100%;
            box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
            border: 1px solid rgba(255, 255, 255, 0.1);
        }

        .logo {
            text-align: center;
            margin-bottom: 24px;
        }

        .logo-img {
            max-width: 200px;
            height: auto;
            margin-bottom: 8px;
            pointer-events: none;
        }

        .logo p {
            color: #888;
            font-size: 0.9rem;
        }

        .form-group {
            margin-bottom: 20px;
        }

        label {
            display: block;
            margin-bottom: 8px;
            font-weight: 500;
            color: #e0e0e0;
        }

        .required::after {
            content: ' *';
            color: #ed1c24;
        }

        .optional {
            color: #888;
            font-size: 0.8rem;
            font-weight: normal;
        }

        input[type="text"] {
            width: 100%;
            padding: 12px 16px;
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 8px;
            background: rgba(0, 0, 0, 0.3);
            color: #e0e0e0;
            font-size: 1rem;
            transition: border-color 0.2s, box-shadow 0.2s;
        }

        input[type="text"]:focus {
            outline: none;
            border-color: #ed1c24;
            box-shadow: 0 0 0 3px rgba(237, 28, 36, 0.2);
        }

        input[type="text"]::placeholder {
            color: #666;
        }

        .help-text {
            margin-top: 6px;
            font-size: 0.8rem;
            color: #888;
        }

        .help-text a {
            color: #ed1c24;
            text-decoration: none;
        }

        .help-text a:hover {
            text-decoration: underline;
        }

        .info-box {
            background: rgba(237, 28, 36, 0.1);
            border: 1px solid rgba(237, 28, 36, 0.3);
            border-radius: 8px;
            padding: 16px;
            margin-bottom: 24px;
        }
        .info-box h3 {
            color: #ed1c24;
            margin-bottom: 8px;
            font-size: 0.95rem;
        }

        .info-box p {
            font-size: 0.85rem;
            line-height: 1.6;
        }

        button, .btn {
            width: 100%;
            padding: 14px;
            background: linear-gradient(135deg, #ed1c24 0%, #c41e3a 100%);
            border: none;
            border-radius: 8px;
            color: white;
            font-size: 1rem;
            font-weight: 600;
            cursor: pointer;
            transition: transform 0.2s, box-shadow 0.2s;
            text-decoration: none;
            display: inline-block;
            text-align: center;
        }
        button:hover, .btn:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 12px rgba(237, 28, 36, 0.4);
        }

        button:active, .btn:active {
            transform: translateY(0);
        }

        button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
            transform: none;
        }

        .device-code {
            font-size: 2.5rem;
            font-weight: bold;
            font-family: 'Courier New', monospace;
            letter-spacing: 0.3em;
            color: #fff;
            background: rgba(237, 28, 36, 0.2);
            border: 2px solid #ed1c24;
            border-radius: 12px;
            padding: 20px 30px;
            margin: 24px auto;
            display: block; 
            width: -moz-fit-content;
            width: fit-content;
            cursor: pointer;
            transition: all 0.2s ease;
        }

        .device-code:hover {
            background: rgba(237, 28, 36, 0.3);
            transform: scale(1.02);
            box-shadow: 0 0 15px rgba(237, 28, 36, 0.4);
        }

        .device-code:active {
            transform: scale(0.98);
        }

        .error {
            background: rgba(220, 53, 69, 0.2);
            border: 1px solid rgba(220, 53, 69, 0.5);
            color: #ff6b6b;
            padding: 12px;
            border-radius: 8px;
            margin-bottom: 20px;
            display: none;
        }
        .error.show {
            display: block;
        }

        .footer {
            text-align: center;
            margin-top: 24px;
            font-size: 0.8rem;
            color: #666;
        }

        .footer a {
            color: #888;
            cursor: pointer;
        }

        .auth-container {
            display: none;
            text-align: center;
        }

        .auth-container.show {
            display: block;
        }
        
        .auth-instructions {
            margin-bottom: 24px;
            line-height: 1.6;
        }

        .auth-instructions p {
            margin-bottom: 12px;
        }

        .auth-instructions .step {
            display: flex;
            align-items: center;
            justify-content: center;
            margin-bottom: 16px;
        }

        .auth-instructions .step-number {
            background: #ed1c24;
            color: white;
            width: 28px;
            height: 28px;
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            font-weight: bold;
            margin-right: 12px;
            flex-shrink: 0;
        }

        .status-message {
            margin-top: 24px;
            padding: 16px;
            border-radius: 8px;
            background: rgba(255, 255, 255, 0.05);
        }

        .status-message.waiting {
            color: #888;
        }

        .status-message.success {
            background: rgba(76, 175, 80, 0.2);
            color: #4CAF50;
        }

        .status-message.error {
            background: rgba(220, 53, 69, 0.2);
            color: #ff6b6b;
        }

        .spinner {
            display: inline-block;
            width: 16px;
            height: 16px;
            border: 2px solid rgba(255,255,255,0.3);
            border-radius: 50%;
            border-top-color: #fff;
            animation: spin 1s ease-in-out infinite;
            margin-right: 8px;
            vertical-align: middle;
        }

        @keyframes spin {
            to { transform: rotate(360deg); }
        }

        .hidden {
            display: none !important;
        }
    "#
}

// =============================================================================
// JavaScript
// =============================================================================

#[allow(clippy::too_many_lines)]
fn script() -> &'static str {
    r"
        let pollInterval = null;
        let pollIntervalMs = 5000;

        document.getElementById('setupForm').addEventListener('submit', async function(e) {
            e.preventDefault();

            const errorDiv = document.getElementById('error');
            const submitBtn = document.getElementById('submitBtn');
            errorDiv.classList.remove('show');

            const formData = new FormData(this);
            const data = Object.fromEntries(formData.entries());

            if (!data.traktUser) {
                errorDiv.textContent = 'Please fill in the Trakt Username field.';
                errorDiv.classList.add('show');
                return;
            }

            submitBtn.disabled = true;
            submitBtn.textContent = 'Connecting...';

            try {
                const response = await fetch('/submit', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(data),
                });

                if (response.ok) {
                    const result = await response.json();
                    if (result.user_code && result.verification_url) {
                        showAuthScreen(result);
                    } else {
                        showSuccessScreen();
                    }
                } else {
                    const errorText = await response.text();
                    errorDiv.textContent = errorText || 'Failed to save configuration. Please try again.';
                    errorDiv.classList.add('show');
                    submitBtn.disabled = false;
                    submitBtn.textContent = 'Login with Trakt';
                }
            } catch (err) {
                errorDiv.textContent = 'Connection error. Please try again.';
                errorDiv.classList.add('show');
                submitBtn.disabled = false;
                submitBtn.textContent = 'Login with Trakt';
            }
        });

        function showAuthScreen(deviceInfo) {
            document.getElementById('setupForm-container').classList.add('hidden');
            document.getElementById('auth-container').classList.add('show');

            document.getElementById('deviceCode').textContent = deviceInfo.user_code;

            const autoUrl = deviceInfo.verification_url + '?code=' + deviceInfo.user_code;
            document.getElementById('traktLink').href = autoUrl;

            const expiresInMinutes = Math.floor(deviceInfo.expires_in / 60);
            document.getElementById('expiresIn').textContent = expiresInMinutes;

            pollIntervalMs = (deviceInfo.interval || 5) * 1000;
            startPolling();
        }

        function showSuccessScreen() {
            document.getElementById('setupForm-container').classList.add('hidden');
            document.getElementById('auth-container').classList.remove('show');
            document.getElementById('success-container').classList.add('show');

            if (pollInterval) {
                clearInterval(pollInterval);
                pollInterval = null;
            }

            // Auto-close tab after a short delay
            setTimeout(() => {
                window.close();
            }, 2000);
        }

        function showError(message) {
            const statusDiv = document.getElementById('statusMessage');
            statusDiv.className = 'status-message error';
            statusDiv.textContent = message;

            if (pollInterval) {
                clearInterval(pollInterval);
                pollInterval = null;
            }
        }

        function startPolling() {
            pollInterval = setInterval(async () => {
                try {
                    const response = await fetch('/status');
                    const result = await response.json();

                    switch (result.status) {
                        case 'success':
                            showSuccessScreen();
                            break;
                        case 'pending':
                            break;
                        case 'denied':
                            showError('Authorization was denied. Please restart Discrakt to try again.');
                            break;
                        case 'expired':
                            showError('The code has expired. Please restart Discrakt to try again.');
                            break;
                        case 'error':
                            showError('An error occurred: ' + (result.message || 'Unknown error'));
                            break;
                    }
                } catch (err) {
                    console.error('Polling error:', err);
                }
            }, pollIntervalMs);
        }
    "
}

// =============================================================================
// HTML Components
// =============================================================================

fn header() -> String {
    format!(
        r#"
        <div class="logo">
            <img src="/logo.svg" alt="{APP_NAME}" class="logo-img">
            <p>{APP_TAGLINE}</p>
        </div>
        "#
    )
}

fn footer() -> String {
    format!(
        r#"
        <div class="footer">
            <p>Configuration will be saved to your system config directory</p>
            <p><a href="{GITHUB_URL}" target="_blank">GitHub</a></p>
        </div>
        "#
    )
}

fn setup_form() -> String {
    let footer = footer();
    format!(
        r#"
        <div id="setupForm-container">
            <div class="info-box">
                <h3>Getting Started</h3>
                <p>Enter your Trakt username to connect your account.</p>
            </div>

            <div class="error" id="error"></div>

            <form id="setupForm" method="POST" action="/submit">
                <div class="form-group">
                    <label for="traktUser" class="required">Trakt Username</label>
                    <input type="text" id="traktUser" name="traktUser"
                           placeholder="Your Trakt username" required
                           autocomplete="username">
                    <p class="help-text">
                        Find it at <a href="{TRAKT_SETTINGS_URL}" target="_blank">trakt.tv/settings</a>
                    </p>
                </div>

                <button type="submit" id="submitBtn">Login with Trakt</button>
            </form>

            {footer}
        </div>
        "#
    )
}

fn auth_screen() -> String {
    format!(
        r##"
        <div id="auth-container" class="auth-container">
            <div class="auth-instructions">
                <p>Click the button below to authorize Discrakt.</p>
                <p style="font-size: 0.9rem; color: #888;">
                    Verify that the code on Trakt matches this one:
                </p>
            </div>

            <div class="device-code" id="deviceCode">--------</div>

            <a id="traktLink" href="#" target="_blank" class="btn">
                Open Trakt & Authorize
            </a>

            <div id="statusMessage" class="status-message waiting">
                <span class="spinner"></span>
                Waiting for authorization...
            </div>

            <div class="footer">
                <p>The code expires in <span id="expiresIn">10</span> minutes</p>
                <p><a href="{GITHUB_URL}" target="_blank">GitHub</a></p>
            </div>
        </div>
        "##
    )
}

fn success_screen() -> String {
    format!(
        r#"
        <div id="success-container" class="auth-container">
            <h2 style="color: {COLOR_SUCCESS}; margin-bottom: 24px;">Authorization Successful!</h2>
            <p style="margin-bottom: 16px;">Your Trakt account has been connected.</p>
            <p style="color: #888;">{APP_NAME} is now starting.</p>
            <p style="margin-top: 20px; color: #666; font-size: 0.9rem;">
                This tab will close automatically...
            </p>
        </div>
        "#
    )
}

// =============================================================================
// Public API
// =============================================================================

/// Returns the main setup page HTML.
///
/// The page includes:
/// - Setup form for credentials
/// - OAuth device code screen (shown after form submission)
/// - Success screen (shown after authorization)
pub fn setup_page() -> String {
    let styles = styles();
    let header = header();
    let setup_form = setup_form();
    let auth_screen = auth_screen();
    let success_screen = success_screen();
    let script = script();
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{APP_NAME} Setup</title>
    <link rel="icon" type="image/png" href="/favicon.png">
    <link rel="shortcut icon" type="image/png" href="/favicon.png">
    <style>{styles}</style>
</head>
<body>
    <div class="container">
        {header}
        {setup_form}
        {auth_screen}
        {success_screen}
    </div>
    <script>{script}</script>
</body>
</html>"#
    )
}

/// Returns a standalone success page HTML (used as fallback).
#[allow(dead_code)]
pub fn success_page() -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Setup Complete - {APP_NAME}</title>
    <style>
        body {{{{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 50%, #0f3460 100%);
            min-height: 100vh;
            display: flex;
            justify-content: center;
            align-items: center;
            color: #e0e0e0;
        }}}}
        .container {{{{
            text-align: center;
            background: rgba(255, 255, 255, 0.05);
            padding: 40px;
            border-radius: 16px;
            max-width: 400px;
        }}}}
        h1 {{{{
            color: {COLOR_SUCCESS};
            margin-bottom: 16px;
        }}}}
        p {{{{
            color: #888;
            margin-bottom: 12px;
        }}}}
    </style>
</head>
<body>
    <div class="container">
        <h1>Setup Complete!</h1>
        <p>Your credentials have been saved successfully.</p>
        <p>{APP_NAME} is now starting. You can close this page.</p>
        <p style="font-size: 0.9rem;">Look for the {APP_NAME} icon in your system tray.</p>
    </div>
</body>
</html>"#
    )
}
