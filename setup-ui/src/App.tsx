import { type FormEvent, useEffect, useState } from "react";
import { ExternalLink, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getStatus, startPlexLogin, submitPlex, submitTrakt } from "@/lib/api";

type AuthInfo = {
  /** The URL the user opens to authorize. */
  link: string;
  /** A short code to display (Trakt only); omitted for Plex. */
  code?: string;
  buttonLabel: string;
  expiresInMinutes: number;
  intervalSeconds: number;
};

type Screen = { name: "setup" } | { name: "auth"; info: AuthInfo } | { name: "success" };

const TAGLINE = "Trakt/Plex to Discord Rich Presence";

export function App() {
  const [screen, setScreen] = useState<Screen>({ name: "setup" });
  const [error, setError] = useState<string | null>(null);

  // While waiting for authorization, poll the server for completion.
  useEffect(() => {
    if (screen.name !== "auth") return;
    const id = setInterval(async () => {
      try {
        const status = await getStatus();
        if (status.status === "success") setScreen({ name: "success" });
        else if (status.status === "denied")
          setError("Authorization was denied. Restart Discrakt to try again.");
        else if (status.status === "expired")
          setError("The code expired. Restart Discrakt to try again.");
        else if (status.status === "error") setError(status.message || "Something went wrong.");
      } catch {
        // transient network error; keep polling
      }
    }, screen.info.intervalSeconds * 1000);
    return () => clearInterval(id);
  }, [screen]);

  return (
    <div className="flex min-h-svh items-center justify-center bg-background p-6 text-foreground">
      <Card className="w-full max-w-md">
        <CardContent className="flex flex-col gap-6 p-8">
          <header className="flex flex-col items-center gap-2 text-center">
            <img src="/logo.svg" alt="Discrakt" className="h-9 select-none" />
            <p className="text-sm text-muted-foreground">{TAGLINE}</p>
          </header>

          {screen.name === "setup" && (
            <SetupScreen
              error={error}
              setError={setError}
              onAuth={(info) => {
                setError(null);
                setScreen({ name: "auth", info });
              }}
              onDone={() => setScreen({ name: "success" })}
            />
          )}
          {screen.name === "auth" && <AuthScreen info={screen.info} error={error} />}
          {screen.name === "success" && <SuccessScreen />}
        </CardContent>
      </Card>
    </div>
  );
}

function ErrorBox({ message }: { message: string }) {
  return (
    <div className="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
      {message}
    </div>
  );
}

type SetupProps = {
  error: string | null;
  setError: (message: string | null) => void;
  onAuth: (info: AuthInfo) => void;
  onDone: () => void;
};

function SetupScreen({ error, setError, onAuth, onDone }: SetupProps) {
  return (
    <Tabs defaultValue="trakt" onValueChange={() => setError(null)} className="gap-5">
      <TabsList className="w-full">
        <TabsTrigger value="trakt">Trakt</TabsTrigger>
        <TabsTrigger value="plex">Plex</TabsTrigger>
      </TabsList>

      {error && <ErrorBox message={error} />}

      <TabsContent value="trakt">
        <TraktForm setError={setError} onAuth={onAuth} onDone={onDone} />
      </TabsContent>
      <TabsContent value="plex">
        <PlexPane setError={setError} onAuth={onAuth} onDone={onDone} />
      </TabsContent>
    </Tabs>
  );
}

function TraktForm({ setError, onAuth, onDone }: Omit<SetupProps, "error">) {
  const [busy, setBusy] = useState(false);

  async function handleLogin() {
    setBusy(true);
    setError(null);
    try {
      const result = await submitTrakt();
      if (result.user_code && result.verification_url) {
        onAuth({
          link: `${result.verification_url}?code=${encodeURIComponent(result.user_code)}`,
          code: result.user_code,
          buttonLabel: "Open Trakt & Authorize",
          expiresInMinutes: Math.floor((result.expires_in ?? 600) / 60),
          intervalSeconds: result.interval ?? 5,
        });
      } else {
        onDone();
      }
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-muted-foreground">
        Connect your Trakt account. You'll approve Discrakt in your browser, then any
        app that scrobbles to Trakt shows up on Discord.
      </p>
      <Button onClick={handleLogin} disabled={busy}>
        {busy && <Loader2 className="animate-spin" />}
        Login with Trakt
      </Button>
    </div>
  );
}

function PlexPane({ setError, onAuth, onDone }: Omit<SetupProps, "error">) {
  const [busy, setBusy] = useState(false);
  const [form, setForm] = useState({ serverUrl: "", token: "", username: "" });

  async function handleLogin() {
    setBusy(true);
    setError(null);
    try {
      const data = await startPlexLogin();
      // Best-effort auto-open; the link is shown regardless of popup blocking.
      window.open(data.authUrl, "_blank", "noopener");
      onAuth({
        link: data.authUrl,
        buttonLabel: "Open Plex & Authorize",
        expiresInMinutes: Math.floor((data.expiresIn ?? 1800) / 60),
        intervalSeconds: data.interval ?? 2,
      });
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  async function handleManual(event: FormEvent) {
    event.preventDefault();
    if (!form.serverUrl.trim() || !form.token.trim()) {
      setError("Please fill in the Plex server URL and token.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await submitPlex(form);
      onDone();
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <Button onClick={handleLogin} disabled={busy}>
        {busy && <Loader2 className="animate-spin" />}
        Login with Plex
      </Button>

      <div className="flex items-center gap-3 text-xs text-muted-foreground">
        <span className="h-px flex-1 bg-border" />
        or enter server details manually
        <span className="h-px flex-1 bg-border" />
      </div>

      <form onSubmit={handleManual} className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <Label htmlFor="serverUrl">Plex server URL</Label>
          <Input
            id="serverUrl"
            placeholder="http://192.168.1.10:32400"
            value={form.serverUrl}
            onChange={(e) => setForm({ ...form, serverUrl: e.target.value })}
          />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="token">Plex token</Label>
          <Input
            id="token"
            placeholder="Your X-Plex-Token"
            value={form.token}
            onChange={(e) => setForm({ ...form, token: e.target.value })}
          />
          <p className="text-xs text-muted-foreground">
            <a
              className="underline"
              href="https://support.plex.tv/articles/204059436-finding-an-authentication-token-x-plex-token/"
              target="_blank"
              rel="noreferrer"
            >
              How to find your token
            </a>
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="plexUsername">Plex username</Label>
          <Input
            id="plexUsername"
            placeholder="Optional (for shared servers)"
            value={form.username}
            onChange={(e) => setForm({ ...form, username: e.target.value })}
          />
        </div>
        <Button type="submit" variant="secondary" disabled={busy}>
          Connect Plex
        </Button>
      </form>
    </div>
  );
}

function AuthScreen({ info, error }: { info: AuthInfo; error: string | null }) {
  return (
    <div className="flex flex-col items-center gap-5 text-center">
      <p className="text-sm text-muted-foreground">
        Click below to authorize Discrakt in your browser.
      </p>

      {info.code && (
        <div className="rounded-lg border border-primary/40 bg-primary/10 px-6 py-4 font-mono text-2xl tracking-widest text-foreground select-text">
          {info.code}
        </div>
      )}

      <Button className="w-full" render={<a href={info.link} target="_blank" rel="noreferrer" />}>
        <ExternalLink />
        {info.buttonLabel}
      </Button>

      {error ? (
        <ErrorBox message={error} />
      ) : (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          Waiting for authorization...
        </div>
      )}

      <p className="text-xs text-muted-foreground">
        The code expires in {info.expiresInMinutes} minutes.
      </p>
    </div>
  );
}

function SuccessScreen() {
  const [seconds, setSeconds] = useState(5);

  useEffect(() => {
    if (seconds <= 0) {
      window.close();
      return;
    }
    const id = setTimeout(() => setSeconds((s) => s - 1), 1000);
    return () => clearTimeout(id);
  }, [seconds]);

  return (
    <div className="flex flex-col items-center gap-3 text-center">
      <h2 className="text-xl font-semibold text-emerald-500">Setup complete!</h2>
      <p className="text-sm text-muted-foreground">Your account has been connected.</p>
      <p className="text-xs text-muted-foreground">
        Discrakt is now starting. This tab will close in {seconds} second{seconds === 1 ? "" : "s"}.
      </p>
    </div>
  );
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : "Connection error. Please try again.";
}

export default App;
