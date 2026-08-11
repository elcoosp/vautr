import { useMemo, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { Switch } from '@/components/ui/switch';
import {
  assessPassword,
  generatePassword,
  isReusedPassword,
  strengthLabel,
  type GeneratorOptions,
} from '@vautr/client-sdk';
import { usePopupStore } from '../store';

export function GeneratorTab() {
  const revealed = usePopupStore((s) => s.revealedPasswords);
  const [length, setLength] = useState(20);
  const [upper, setUpper] = useState(true);
  const [lower, setLower] = useState(true);
  const [digits, setDigits] = useState(true);
  const [symbols, setSymbols] = useState(true);
  const [excludeAmbiguous, setExcludeAmbiguous] = useState(true);
  const [password, setPassword] = useState('');
  const [copied, setCopied] = useState(false);

  const assessment = useMemo(() => assessPassword(password), [password]);
  const reused = useMemo(() => isReusedPassword(password, revealed), [password, revealed]);

  function generate(): void {
    const options: GeneratorOptions = {
      length,
      upper,
      lower,
      digits,
      symbols,
      excludeAmbiguous,
    };
    setPassword(generatePassword(options));
    setCopied(false);
  }

  async function copy(): Promise<void> {
    if (!password) return;
    await navigator.clipboard.writeText(password);
    setCopied(true);
    toast.success('Password copied.');
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Password generator</CardTitle>
          <CardDescription className="text-xs">
            Create a strong, random password. We flag weak and reused passwords.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1">
            <Label>Generated password</Label>
            <div className="flex gap-2">
              <Input readOnly value={password} placeholder="Generate one to begin" />
              <Button variant="outline" onClick={() => void generate()}>
                Generate
              </Button>
            </div>
            {password ? (
              <div className="flex items-center gap-2 pt-1">
                <Badge
                  variant={
                    assessment.score <= 1
                      ? 'destructive'
                      : assessment.score === 2
                        ? 'secondary'
                        : 'default'
                  }
                >
                  {strengthLabel(assessment.score)} · {assessment.entropy} bits
                </Badge>
                {reused ? (
                  <Badge variant="destructive">Reused</Badge>
                ) : (
                  <Badge variant="outline">Unique</Badge>
                )}
              </div>
            ) : null}
          </div>

          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="outline" disabled={!password} onClick={() => void copy()}>
              {copied ? 'Copied ✓' : 'Copy'}
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={!password}
              onClick={() => setPassword('')}
            >
              Clear
            </Button>
          </div>

          <Separator />

          <div className="space-y-1">
            <Label>Length: {length}</Label>
            <Input
              type="range"
              min={8}
              max={64}
              value={length}
              onChange={(e) => setLength(Number(e.target.value))}
            />
          </div>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="gen-upper">Uppercase (A–Z)</Label>
              <Switch id="gen-upper" checked={upper} onCheckedChange={setUpper} />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="gen-lower">Lowercase (a–z)</Label>
              <Switch id="gen-lower" checked={lower} onCheckedChange={setLower} />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="gen-digits">Digits (0–9)</Label>
              <Switch id="gen-digits" checked={digits} onCheckedChange={setDigits} />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="gen-symbols">Symbols</Label>
              <Switch id="gen-symbols" checked={symbols} onCheckedChange={setSymbols} />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="gen-ambiguous">Exclude ambiguous</Label>
              <Switch
                id="gen-ambiguous"
                checked={excludeAmbiguous}
                onCheckedChange={setExcludeAmbiguous}
              />
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
