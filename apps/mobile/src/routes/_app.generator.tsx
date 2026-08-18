import { createFileRoute } from '@tanstack/react-router';
import {
  analyzePassword,
  DEFAULT_GENERATOR_OPTIONS,
  type GeneratorOptions,
  generatePassword,
  type StrengthResult,
} from '@vautr/ui-logic';
import { RefreshCw } from 'lucide-react-native';
import { useMemo, useState } from 'react';
import { Switch, View } from 'react-native';
import { Alert, AlertDescription, AlertTitle } from '../../components/ui/alert';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { ThemedIcon } from '../../components/ui/icon';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Separator } from '../../components/ui/separator';
import { Text } from '../../components/ui/text';

export const Route = createFileRoute('/_app/generator')({
  component: GeneratorScreen,
});

const OPTION_LABELS: { key: keyof GeneratorOptions; label: string }[] = [
  { key: 'uppercase', label: 'Uppercase (A–Z)' },
  { key: 'lowercase', label: 'Lowercase (a–z)' },
  { key: 'digits', label: 'Digits (2–9)' },
  { key: 'symbols', label: 'Symbols (!@#…)' },
  { key: 'avoidAmbiguous', label: 'Avoid ambiguous (I, O, 1, 0)' },
];

function scoreVariant(
  score: StrengthResult['score'],
): 'default' | 'secondary' | 'outline' | 'destructive' {
  switch (score) {
    case 'strong':
    case 'good':
      return 'default';
    case 'fair':
      return 'secondary';
    default:
      return 'destructive';
  }
}

function GeneratorScreen() {
  const [options, setOptions] = useState<GeneratorOptions>(DEFAULT_GENERATOR_OPTIONS);
  const [password, setPassword] = useState(() => generatePassword(DEFAULT_GENERATOR_OPTIONS));
  const [known, setKnown] = useState('');

  const knownList = useMemo(() => known.split(/[\s,]+/).filter(Boolean), [known]);
  const analysis = useMemo(() => analyzePassword(password, knownList), [password, knownList]);

  const regenerate = () => {
    try {
      setPassword(generatePassword(options));
    } catch {
      // at least one class required; ignore
    }
  };

  const toggle = (key: keyof GeneratorOptions) => {
    const next = { ...options, [key]: !options[key] } as GeneratorOptions;
    setOptions(next);
    try {
      setPassword(generatePassword(next));
    } catch {
      // ignore invalid transient state
    }
  };

  return (
    <View className="gap-5">
      <View className="gap-0.5">
        <Text variant="h2">Generator</Text>
        <Text variant="muted">Create a strong, unique password.</Text>
      </View>

      <Card className="gap-4 p-4">
        <View className="flex-row items-center justify-between gap-3">
          <Text variant="large" className="flex-1 font-mono">
            {password}
          </Text>
          <Button size="sm" onPress={regenerate}>
            <ThemedIcon icon={RefreshCw} size={16} tone="primaryForeground" />
            <ButtonText className="ml-1">Generate</ButtonText>
          </Button>
        </View>

        <View className="flex-row flex-wrap items-center gap-2">
          <Badge variant={scoreVariant(analysis.score)}>{analysis.score}</Badge>
          <Badge variant="outline">{analysis.entropyBits} bits</Badge>
          {analysis.isCommon ? <Badge variant="destructive">common</Badge> : null}
          {analysis.isReused ? <Badge variant="destructive">reused</Badge> : null}
        </View>

        {analysis.suggestions.length > 0 ? (
          <Alert variant="warning">
            <AlertTitle>Suggestions</AlertTitle>
            <AlertDescription>
              {analysis.suggestions.map((s) => `• ${s}\n`).join('')}
            </AlertDescription>
          </Alert>
        ) : null}
      </Card>

      <Card className="gap-3 p-4">
        <Text variant="label">Options</Text>
        <Separator />
        {OPTION_LABELS.map(({ key, label }) => (
          <View key={key} className="flex-row items-center justify-between">
            <Text variant="p">{label}</Text>
            <Switch value={Boolean(options[key])} onValueChange={() => toggle(key)} />
          </View>
        ))}
      </Card>

      <Card className="gap-2 p-4">
        <Label htmlFor="known-passwords">Known passwords</Label>
        <Input
          id="known-passwords"
          value={known}
          onChangeText={setKnown}
          placeholder="Comma or space separated"
          autoCapitalize="none"
          autoCorrect={false}
        />
        <Text variant="tiny">Detect whether a generated password is reused or common.</Text>
      </Card>
    </View>
  );
}
