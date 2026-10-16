const steps = ["Анализ", "Выбор файлов", "Подтверждение", "Результат"];
export function CleanupSteps({ step }: { step: 1 | 2 | 3 | 4 }) {
  return <ol className="step-list" aria-label="Этапы очистки">{steps.map((label, index) => <li key={label} aria-current={step === index + 1 ? "step" : undefined} className={step === index + 1 ? "text-txt font-semibold" : ""}><span>{index + 1}</span>{label}</li>)}</ol>;
}
