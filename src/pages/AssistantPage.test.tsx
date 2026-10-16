import { renderToStaticMarkup } from "react-dom/server";
import { describe,expect,it } from "vitest";
import { AssistantPage,ContextReview } from "./AssistantPage";
describe("assistant interface",()=>{
  it("keeps missing model and desktop state honest and inference disabled",()=>{
    const html=renderToStaticMarkup(<AssistantPage onNavigate={()=>{}}/>);
    expect(html).toContain("Модель не выбрана");expect(html).toContain("Основные функции CleanCore доступны");
    expect(html).not.toContain("Объяснить локально");
    expect(html).not.toContain("<textarea");
    expect(html).toContain("Сначала выберите модель");
    expect(html).toContain("Модель не загружается из интернета");
  });
  it("renders unknown metrics as missing and escapes untrusted scan labels",()=>{
    const html=renderToStaticMarkup(<ContextReview summary={{metrics:{cpu_percent:null,memory_percent:null,disk_free_bytes:null},scan:{category:'<script>bad()</script>',candidates:1,bytes:"10",complete:false},journal:[{status:"Unknown",item_count:1,unknown_count:1,failed_count:0}]}}/>);
    expect(html).toContain("нет данных");expect(html).not.toContain("<script>");expect(html).toContain("&lt;script&gt;");expect(html).toContain("неполный снимок");expect(html).toContain("неизвестных исходов 1");
  });
});
