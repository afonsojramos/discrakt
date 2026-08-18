import { type ReactNode, type SyntheticEvent, useEffect, useRef, useState } from "react";
import { Check, ChevronDown, ExternalLink, Loader2 } from "lucide-react";

import { JellyfinIcon, PlexIcon, TraktIcon } from "@/components/brand-icons";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  continueSetup,
  finishSetup,
  getStatus,
  startJellyfinLogin,
  startPlexLogin,
  submitJellyfin,
  submitPlex,
  submitTrakt,
  submitTraktPublic,
} from "@/lib/api";

/** A source the wizard can connect, as shown to the user. */
type SourceName = "Trakt" | "Plex" | "Jellyfin";

type AuthInfo = {
  /** The source this authorization belongs to. */
  source: SourceName;
  /** The URL the user opens to authorize. */
  link: string;
  /** A short code to display (Trakt device code, Jellyfin Quick Connect). */
  code?: string;
  buttonLabel: string;
  /** Optional override for the instruction text above the code/button. */
  hint?: string;
  expiresInMinutes: number;
  intervalSeconds: number;
};

const TAGLINE = "Trakt / Plex / Jellyfin to Discord Rich Presence";

type Screen = { name: "setup" } | { name: "auth"; info: AuthInfo } | { name: "success" };

export function App() {
  const [screen, setScreen] = useState<Screen>({ name: "setup" });
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState<SourceName[]>([]);

  const markConnected = (source: SourceName) => {
    setConnected((current) => (current.includes(source) ? current : [...current, source]));
    setScreen({ name: "success" });
  };

  // While waiting for authorization, poll the server for completion.
  useEffect(() => {
    if (screen.name !== "auth") return;
    let cancelled = false;
    const check = async () => {
      try {
        const status = await getStatus();
        if (cancelled) return;
        if (status.status === "success") markConnected(screen.info.source);
        else if (status.status === "denied")
          setError("Authorization was denied. Restart Discrakt to try again.");
        else if (status.status === "expired")
          setError("The code expired. Restart Discrakt to try again.");
        else if (status.status === "error") setError(status.message || "Something went wrong.");
      } catch {
        // transient network error; keep polling
      }
    };
    void check();
    const id = setInterval(check, screen.info.intervalSeconds * 1000);
    // Browsers throttle background-tab timers, so the user authorizing in another
    // tab wouldn't see completion until they return. Re-check the moment this tab
    // regains focus to catch up immediately.
    const onVisible = () => {
      if (document.visibilityState === "visible") void check();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      cancelled = true;
      clearInterval(id);
      document.removeEventListener("visibilitychange", onVisible);
    };
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
              connected={connected}
              onAuth={(info) => {
                setError(null);
                setScreen({ name: "auth", info });
              }}
              onDone={markConnected}
            />
          )}
          {screen.name === "auth" && <AuthScreen info={screen.info} error={error} />}
          {screen.name === "success" && (
            <SuccessScreen
              connected={connected}
              onAddAnother={() => {
                setError(null);
                setScreen({ name: "setup" });
              }}
            />
          )}
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
  onDone: (source: SourceName) => void;
};

type SetupScreenProps = SetupProps & { connected: SourceName[] };

function SetupScreen({ error, setError, connected, onAuth, onDone }: SetupScreenProps) {
  // Land on the first source that is not connected yet, so returning here to
  // add a second source does not reopen the one just finished.
  const firstUnconnected =
    (["trakt", "plex", "jellyfin"] as const).find((tab) => !connected.includes(labelOf(tab))) ??
    "trakt";

  return (
    <Tabs defaultValue={firstUnconnected} onValueChange={() => setError(null)} className="gap-5">
      <TabsList className="w-full">
        <TabsTrigger value="trakt">
          {connected.includes("Trakt") ? <Check className="text-emerald-500" /> : <TraktIcon />}
          Trakt
        </TabsTrigger>
        <TabsTrigger value="plex">
          {connected.includes("Plex") ? <Check className="text-emerald-500" /> : <PlexIcon />}
          Plex
        </TabsTrigger>
        <TabsTrigger value="jellyfin">
          {connected.includes("Jellyfin") ? (
            <Check className="text-emerald-500" />
          ) : (
            <JellyfinIcon />
          )}
          Jellyfin
        </TabsTrigger>
      </TabsList>

      {error && <ErrorBox message={error} />}

      <TabsContent value="trakt">
        <TraktForm setError={setError} onAuth={onAuth} onDone={onDone} />
      </TabsContent>
      <TabsContent value="plex">
        <PlexPane setError={setError} onAuth={onAuth} onDone={onDone} />
      </TabsContent>
      <TabsContent value="jellyfin">
        <JellyfinPane setError={setError} onAuth={onAuth} onDone={onDone} />
      </TabsContent>
    </Tabs>
  );
}

function labelOf(tab: "trakt" | "plex" | "jellyfin"): SourceName {
  if (tab === "trakt") return "Trakt";
  return tab === "plex" ? "Plex" : "Jellyfin";
}

function Advanced({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Collapsible className="flex flex-col">
      <CollapsibleTrigger className="group flex items-center gap-1 self-start text-xs text-muted-foreground hover:text-foreground">
        <ChevronDown className="size-3 transition-transform group-data-panel-open:rotate-180" />
        {label}
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-3">{children}</CollapsibleContent>
    </Collapsible>
  );
}

function TraktForm({ setError, onAuth, onDone }: Omit<SetupProps, "error">) {
  const [busy, setBusy] = useState(false);
  const [username, setUsername] = useState("");

  async function handleLogin() {
    setBusy(true);
    setError(null);
    try {
      const result = await submitTrakt();
      if (result.user_code && result.verification_url) {
        onAuth({
          source: "Trakt",
          link: `${result.verification_url}?code=${encodeURIComponent(result.user_code)}`,
          code: result.user_code,
          buttonLabel: "Open Trakt & Authorize",
          expiresInMinutes: Math.floor((result.expires_in ?? 600) / 60),
          intervalSeconds: result.interval ?? 5,
        });
      } else {
        onDone("Trakt");
      }
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  async function handlePublic(event: SyntheticEvent) {
    event.preventDefault();
    if (!username.trim()) {
      setError("Please enter your Trakt username.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await submitTraktPublic(username.trim());
      onDone("Trakt");
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-muted-foreground">
        Connect your Trakt account. You'll approve Discrakt in your browser, then any app that
        scrobbles to Trakt shows up on Discord.
      </p>
      <Button onClick={handleLogin} disabled={busy}>
        {busy && <Loader2 className="animate-spin" />}
        Login with Trakt
      </Button>

      <Advanced label="Use a public profile instead">
        <form onSubmit={handlePublic} className="flex flex-col gap-3">
          <p className="text-xs text-muted-foreground">
            Skip logging in and read your public Trakt watching status by username.
          </p>
          <div className="flex flex-col gap-2">
            <Label htmlFor="traktPublicUser">Trakt username</Label>
            <Input
              id="traktPublicUser"
              autoComplete="username"
              placeholder="Your Trakt username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <Button type="submit" variant="secondary" disabled={busy}>
            Use public profile
          </Button>
        </form>
      </Advanced>
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
        source: "Plex",
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

  async function handleManual(event: SyntheticEvent) {
    event.preventDefault();
    if (!form.serverUrl.trim() || !form.token.trim()) {
      setError("Please fill in the Plex server URL and token.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await submitPlex(form);
      onDone("Plex");
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-muted-foreground">
        Connect to your Plex Media Server and mirror your active session.
      </p>
      <Button onClick={handleLogin} disabled={busy}>
        {busy && <Loader2 className="animate-spin" />}
        Login with Plex
      </Button>

      <Advanced label="Enter server details manually">
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
      </Advanced>
    </div>
  );
}

function JellyfinPane({ setError, onAuth, onDone }: Omit<SetupProps, "error">) {
  const [busy, setBusy] = useState(false);
  const [serverUrl, setServerUrl] = useState("");
  const [manual, setManual] = useState({ serverUrl: "", apiKey: "", username: "" });

  async function handleLogin() {
    if (!serverUrl.trim()) {
      setError("Please enter your Jellyfin server URL.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const data = await startJellyfinLogin(serverUrl.trim());
      const base = serverUrl.trim().replace(/\/$/, "");
      onAuth({
        source: "Jellyfin",
        code: data.code,
        // The Quick Connect page prefills its input from `?code=`, so the user
        // only has to click Authorize (the code below is a manual fallback).
        link: `${base}/web/#/quickconnect?code=${encodeURIComponent(data.code)}`,
        buttonLabel: "Open Jellyfin Quick Connect",
        hint: "Click below and approve in Jellyfin (the code is prefilled):",
        expiresInMinutes: 5,
        intervalSeconds: data.interval ?? 2,
      });
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  async function handleManual(event: SyntheticEvent) {
    event.preventDefault();
    if (!manual.serverUrl.trim() || !manual.apiKey.trim()) {
      setError("Please fill in the server URL and API key.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await submitJellyfin(manual);
      onDone("Jellyfin");
    } catch (err) {
      setError(messageOf(err));
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-muted-foreground">
        Connect to your Jellyfin server with Quick Connect and mirror your active session.
      </p>
      <div className="flex flex-col gap-2">
        <Label htmlFor="jellyfinServer">Jellyfin server URL</Label>
        <Input
          id="jellyfinServer"
          placeholder="http://192.168.1.10:8096"
          value={serverUrl}
          onChange={(e) => setServerUrl(e.target.value)}
        />
      </div>
      <Button onClick={handleLogin} disabled={busy}>
        {busy && <Loader2 className="animate-spin" />}
        Login with Jellyfin
      </Button>

      <Advanced label="Use an API key instead">
        <form onSubmit={handleManual} className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="jfServerUrl">Jellyfin server URL</Label>
            <Input
              id="jfServerUrl"
              placeholder="http://192.168.1.10:8096"
              value={manual.serverUrl}
              onChange={(e) => setManual({ ...manual, serverUrl: e.target.value })}
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="jfApiKey">API key</Label>
            <Input
              id="jfApiKey"
              placeholder="Jellyfin API key"
              value={manual.apiKey}
              onChange={(e) => setManual({ ...manual, apiKey: e.target.value })}
            />
            <p className="text-xs text-muted-foreground">Dashboard → API Keys</p>
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="jfUsername">Jellyfin username</Label>
            <Input
              id="jfUsername"
              placeholder="Your Jellyfin username"
              value={manual.username}
              onChange={(e) => setManual({ ...manual, username: e.target.value })}
            />
          </div>
          <Button type="submit" variant="secondary" disabled={busy}>
            Connect Jellyfin
          </Button>
        </form>
      </Advanced>
    </div>
  );
}

function AuthScreen({ info, error }: { info: AuthInfo; error: string | null }) {
  return (
    <div className="flex flex-col items-center gap-5 text-center">
      <p className="text-sm text-muted-foreground">
        {info.hint ?? "Click below to authorize Discrakt in your browser."}
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

const FINISH_COUNTDOWN_SECONDS = 10;

function SuccessScreen({
  connected,
  onAddAnother,
}: {
  connected: SourceName[];
  onAddAnother: () => void;
}) {
  const [seconds, setSeconds] = useState(FINISH_COUNTDOWN_SECONDS);
  // Only count down while the tab is in focus, so a user who authorized in
  // another tab actually sees the success state before this one closes.
  const [visible, setVisible] = useState(() => document.visibilityState === "visible");
  const [staying, setStaying] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onVisibility = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  const finished = useRef(false);
  const finish = () => {
    // The server stops listening right after this, so fire it once and ignore
    // the inevitable connection error from the socket closing.
    if (finished.current) return;
    finished.current = true;
    void finishSetup().catch(() => {});
    window.close();
  };

  useEffect(() => {
    if (staying) return;
    if (seconds <= 0) {
      finish();
      return;
    }
    if (!visible) return;
    const id = setTimeout(() => setSeconds((s) => s - 1), 1000);
    return () => clearTimeout(id);
  }, [seconds, visible, staying]);

  async function handleAddAnother() {
    // Stop the countdown first: the server must stay up for the next flow.
    setStaying(true);
    try {
      await continueSetup();
      onAddAnother();
    } catch {
      // Setup already ended; nothing more can be connected in this run.
      setError("Setup has already finished. Restart Discrakt to add another source.");
      setStaying(false);
    }
  }

  const remaining = (["Trakt", "Plex", "Jellyfin"] as const).filter(
    (source) => !connected.includes(source),
  );

  return (
    <div className="flex flex-col items-center gap-4 text-center">
      <h2 className="text-xl font-semibold text-emerald-500">
        {connected.length > 1 ? "Sources connected!" : "Setup complete!"}
      </h2>

      <ul className="flex flex-col gap-1 text-sm text-muted-foreground">
        {connected.map((source) => (
          <li key={source} className="flex items-center justify-center gap-2">
            <Check className="size-4 text-emerald-500" />
            {source} connected
          </li>
        ))}
      </ul>

      {remaining.length > 0 && (
        <p className="text-xs text-muted-foreground">
          Streaming from more than one? Add {formatList(remaining)} too and Discrakt shows whichever
          one is playing.
        </p>
      )}

      {error && <ErrorBox message={error} />}

      <div className="flex w-full flex-col gap-2">
        {remaining.length > 0 && (
          <Button
            variant="secondary"
            onClick={handleAddAnother}
            disabled={staying || error !== null}
          >
            {staying && <Loader2 className="animate-spin" />}
            Add another source
          </Button>
        )}
        <Button onClick={finish}>Start Discrakt</Button>
      </div>

      <p className="text-xs text-muted-foreground">
        {staying
          ? "Pick another source above."
          : `Starting automatically in ${seconds} second${seconds === 1 ? "" : "s"}.`}
      </p>
    </div>
  );
}

function formatList(items: readonly string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} or ${items[items.length - 1]}`;
}

function messageOf(err: unknown): string {
  return err instanceof Error ? err.message : "Connection error. Please try again.";
}

export default App;
