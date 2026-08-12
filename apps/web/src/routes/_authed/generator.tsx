import { createFileRoute } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import {
  DEFAULT_GENERATOR_OPTIONS,
  analyzePassword,
  generatePassword,
  type GeneratorOptions,
} from '@vautr/ui-logic';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { Copy, RefreshCw } from 'lucide-react';
import { toast } from 'sonner';

export const Route = createFileRoute('/_authed/generator')({
  component: GeneratorPage,
});

function GeneratorPage() {
  const [options, setOptions] = useState<GeneratorOptions>(DEFAULT_GENERATOR_OPTIONS);
  const [password, setPassword] = useState(() => generatePassword(DEFAULT_GENERATOR_OPTIONS));
  const [known, setKnown] = useState('');

  const knownList = useMemo(() => known.split(/[\s,]+/).filter(Boolean), [known]);
  const analysis = useMemo(() => analyzePassword(password, knownList), [password, knownList]);

  const regenerate = () => {
    try {
      setPassword(generatePassword(options));
    } catch {
      toast.error('Select at least one character class');
    }
  };

  const copy = () => {
    void navigator.clipboard.writeText(password).then(() => toast.success('Password copied'));
  };

  const scoreColors: Record<string, string> = {
    weak: 'bg-danger',
    fair: 'bg-warn',
    good: 'bg-accent',
    strong: 'bg-green-500',
  };

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold text-text">Password generator</h1>
        <p className="text-sm text-text-muted">
          Generate strong passwords and detect weak or reused ones.
        </p>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Generator</CardTitle>
            <CardDescription>
              Options for a cryptographically-secure random password.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex gap-2">
              <Input
                value={password}
                readOnly
                className="font-mono"
                aria-label="Generated password"
              />
              <Button variant="outline" size="icon" onClick={copy} aria-label="Copy password">
                <Copy className="size-4" aria-hidden="true" />
              </Button>
              <Button variant="outline" size="icon" onClick={regenerate} aria-label="Regenerate">
                <RefreshCw className="size-4" aria-hidden="true" />
              </Button>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="length">Length: {options.length}</Label>
              <Input
                id="length"
                type="range"
                min={8}
                max={64}
                value={options.length}
                onChange={(e) => setOptions((o) => ({ ...o, length: Number(e.target.value) }))}
              />
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <SwitchRow
                label="Uppercase"
                checked={options.uppercase}
                onChange={(v) => setOptions((o) => ({ ...o, uppercase: v }))}
              />
              <SwitchRow
                label="Lowercase"
                checked={options.lowercase}
                onChange={(v) => setOptions((o) => ({ ...o, lowercase: v }))}
              />
              <SwitchRow
                label="Digits"
                checked={options.digits}
                onChange={(v) => setOptions((o) => ({ ...o, digits: v }))}
              />
              <SwitchRow
                label="Symbols"
                checked={options.symbols}
                onChange={(v) => setOptions((o) => ({ ...o, symbols: v }))}
              />
              <SwitchRow
                label="Avoid ambiguous"
                checked={!!options.avoidAmbiguous}
                onChange={(v) => setOptions((o) => ({ ...o, avoidAmbiguous: v }))}
              />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Weak / reused detection</CardTitle>
            <CardDescription>
              Paste a candidate password; we check its strength and reuse.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="check-password">Password to check</Label>
              <Input
                id="check-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="font-mono"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="known-passwords">Known passwords (comma separated)</Label>
              <Input
                id="known-passwords"
                value={known}
                onChange={(e) => setKnown(e.target.value)}
                placeholder="hunter2, mypassword, 123456"
              />
            </div>

            <div className="rounded-md border border-border bg-surface-raised p-4">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-text">Strength</span>
                <div className="flex items-center gap-2">
                  <Badge
                    className={
                      analysis.score === 'strong'
                        ? 'bg-green-600 text-white'
                        : analysis.score === 'good'
                          ? 'bg-accent'
                          : analysis.score === 'fair'
                            ? 'bg-warn text-black'
                            : 'bg-danger text-white'
                    }
                  >
                    {analysis.score}
                  </Badge>
                  <span className="text-xs text-text-muted">{analysis.entropyBits} bits</span>
                </div>
              </div>
              <div className="mt-2 h-2 overflow-hidden rounded-full bg-border">
                <div
                  className={`h-full ${scoreColors[analysis.score] ?? 'bg-border'}`}
                  style={{ width: `${Math.min(100, analysis.entropyBits)}%` }}
                />
              </div>
              {analysis.isReused ? (
                <p className="mt-2 text-sm font-medium text-warn">⚠ Reused password detected.</p>
              ) : null}
              {analysis.isCommon ? (
                <p className="mt-1 text-sm font-medium text-danger">
                  This is a common, easily-guessed password.
                </p>
              ) : null}
              {analysis.suggestions.length > 0 ? (
                <ul className="mt-2 list-disc pl-4 text-sm text-text-muted">
                  {analysis.suggestions.map((s) => (
                    <li key={s}>{s}</li>
                  ))}
                </ul>
              ) : null}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function SwitchRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-md border border-border bg-surface-raised px-3 py-2">
      <Label className="cursor-pointer text-sm text-text">{label}</Label>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </div>
  );
}
