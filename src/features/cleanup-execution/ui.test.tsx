import { renderToStaticMarkup } from "react-dom/server";
import { describe, it, expect, vi } from "vitest";
import { ConfirmationReview } from "./ConfirmationReview";
import { OutcomeView, formatBytes } from "./OutcomeView";
import type { CleanupConfirmation, CleanupOutcome, ItemStatus } from "./types";
const confirmation: CleanupConfirmation = { confirmation_id: "token", preview_id: "preview", expires_at_ms: 1,
  selected_bytes: "9007199254740993", consequences: "Восстановление не гарантируется", selected: [{ object_id: "id", display_path: "<script>owned-test.tmp</script>", size_bytes: "42", reason: "test" }] };
function renderConfirmation(acknowledged: boolean, allowed: boolean, expired: boolean) {
  return renderToStaticMarkup(<ConfirmationReview confirmation={confirmation} acknowledged={acknowledged} allowed={allowed} expired={expired} busy={false} onAcknowledge={vi.fn()} onExecute={vi.fn()} onBack={vi.fn()}/>);
}
describe("explicit cleanup confirmation", () => {
  it("blocks execution without acknowledgement, capability or valid lifetime", () => {
    for (const args of [[false,true,false],[true,false,false],[true,true,true]] as const) {
      const html=renderConfirmation(args[0],args[1],args[2]);
      expect(html).toMatch(/<button disabled=""[^>]*>Подтвердить и отправить/);
    }
    expect(renderConfirmation(true,true,false)).not.toMatch(/<button disabled=""[^>]*>Подтвердить и отправить/);
  });
  it("shows exact paths as escaped text and preserves large byte counts", () => {
    const html=renderConfirmation(false,true,false); expect(html).toContain("&lt;script&gt;owned-test.tmp&lt;/script&gt;");
    expect(html).not.toContain("<script>"); expect(formatBytes(confirmation.selected_bytes).replace(/\D/g," ").trim().split(/\s+/).join("")).toBe("9007199254740993");
  });
});
describe("honest outcome states", () => {
  it("distinguishes all per-object outcomes and warns about uncertain journal", () => {
    const states: ItemStatus[]=["Pending","Recycled","Deleted","Skipped","Failed","Cancelled","Unknown"];
    const o: CleanupOutcome={operation_id:"operation",preview_id:"preview",category:"user-temp-v1",started_at_ms:1,status:"JournalFailed",journal_ok:false,recycled_bytes:"42",items:states.map((status,i)=>({object_id:String(i),display_path:`test-${i}.tmp`,size_bytes:"42",status,reason_code:"test"}))};
    const html=renderToStaticMarkup(<OutcomeView outcome={o}/>);
    for(const label of ["Ожидает","В Корзине","Пропущен","Ошибка","Отменён","Исход неизвестен","Сбой журнала","не освобождённое место","не полностью записан"]) expect(html).toContain(label);
  });
});

it("requires explicit permanent deletion acknowledgement and names the irreversible action",()=>{
  const html=renderToStaticMarkup(<ConfirmationReview confirmation={{...confirmation,irreversible:true}} acknowledged={false} allowed busy={false} expired={false} onAcknowledge={vi.fn()} onExecute={vi.fn()} onBack={vi.fn()}/>);
  expect(html).toContain("безвозвратно, без Корзины и восстановления");
  expect(html).toMatch(/<button disabled=""[^>]*>Удалить выбранное безвозвратно/);
});
