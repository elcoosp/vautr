import { usePopupStore } from '@/popup/store';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

/**
 * Extension dashboard — parity with the web/desktop/mobile dashboards.
 * Shows aggregate counts over the locally-synced vault (no extra server
 * round-trips) so the popup has a home surface consistent with the other
 * clients (VTR-104 homogeneity, G4).
 */
export function DashboardTab() {
  const projects = usePopupStore((s) => s.projects);
  const secrets = usePopupStore((s) => s.secrets);

  const stats = [
    { label: 'Projects', value: projects.length },
    { label: 'Secrets', value: secrets.length },
  ];

  return (
    <div className="space-y-4 p-4">
      <div>
        <h2 className="text-lg font-semibold">Dashboard</h2>
        <p className="text-sm text-muted-foreground">
          Local snapshot of your synced vault.
        </p>
      </div>
      <div className="grid grid-cols-2 gap-3">
        {stats.map((s) => (
          <Card key={s.label}>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm text-muted-foreground">{s.label}</CardTitle>
            </CardHeader>
            <CardContent>
              <span className="text-3xl font-bold">{s.value}</span>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
