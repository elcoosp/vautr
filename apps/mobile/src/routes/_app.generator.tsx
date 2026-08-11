import { createFileRoute } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import { Switch, Text, View } from 'react-native';
import { RefreshCw } from 'lucide-react-native';

import {
  DEFAULT_GENERATOR_OPTIONS,
  analyzePassword,
  generatePassword,
  type GeneratorOptions,
  type StrengthResult,
} from '@vautr/ui-logic';

import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';

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

function scoreVariant(score: StrengthResult['score']): 'default' | 'secondary' | 'outline' | 'destructive' {
  switch (score) {
    case 'strong':
      return 'default';
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
    <View className="gap-4">
      <Text className="text-lg font-semibold text-foreground">Password generator</Text>

      <Card className="p-4 gap-3">
        <View className="flex-row items-center justify-between">
          <View className="flex-1 pr-2">
            <Text className="font-mono text-lg text-foreground" selectable>
              {password}
            </Text>
          </View>
          <Button size="sm" onPress={regenerate}>
            <RefreshCw size={16} className="text-primary-foreground" />
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
          <View className="gap-1">
            {analysis.suggestions.map((suggestion) => (
              <Text key={suggestion} className="text-xs text-muted-foreground">
                • {suggestion}
              </Text>
            ))}
          </View>
        ) : null}
      </Card>

      <Card className="p-4 gap-3">
        <Text className="text-sm font-medium text-foreground">Options</Text>
        {OPTION_LABELS.map(({ key, label }) => (
          <View key={key} className="flex-row items-center justify-between">
            <Text className="text-sm text-foreground">{label}</Text>
            <Switch value={Boolean(options[key])} onValueChange={() => toggle(key)} />
          </View>
        ))}
      </Card>

      <Card className="p-4 gap-2">
        <Label htmlFor="known-passwords">Known passwords (weak / reused detection)</Label>
        <Input
          id="known-passwords"
          value={known}
          onChangeText={setKnown}
          placeholder="Comma or space separated"
          autoCapitalize="none"
          autoCorrect={false}
        />
        <Text className="text-xs text-muted-foreground">
          Detect whether a generated password is reused or common.
        </Text>
      </Card>
    </View>
  );
}
