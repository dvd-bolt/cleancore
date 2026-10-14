import { AuditHistory } from "../features/cleanup-execution/AuditHistory";
export const JournalPage = () => <div className="space-y-6">
  <div className="pb-3 border-b border-line/40">
    <p className="font-mono text-[10px] tracking-widest text-dim">ЛОКАЛЬНАЯ ИСТОРИЯ</p>
    <h2 className="font-heading text-2xl font-bold text-txt">Каждый исход. С объяснением.</h2>
  </div>
  <AuditHistory />
</div>;
