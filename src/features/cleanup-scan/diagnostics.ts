export const scanReasons: Record<string,string> = {
  ENTRY_LIMIT: "Достигнут лимит просмотренных объектов", CANDIDATE_LIMIT: "Достигнут лимит кандидатов",
  DEPTH_LIMIT: "Достигнут лимит глубины", TIME_LIMIT: "Достигнут лимит времени",
  READ_DIRECTORY_FAILED: "Каталог недоступен", READ_ENTRY_FAILED: "Ошибка чтения записи каталога",
  OUTSIDE_ROOT: "Объект вне разрешённого корня", REPARSE_OR_UNAVAILABLE: "Ссылка или недоступный объект",
  REPARSE_POINT: "Ссылка или точка повторной обработки", HARD_LINK: "Файл имеет несколько имён",
  FILE_IN_USE: "Файл занят", ACCESS_DENIED: "Доступ запрещён", NOT_FOUND: "Объект исчез",
  TYPE_UNAVAILABLE: "Тип объекта недоступен", NOT_REGULAR_FILE: "Не обычный файл",
  WRONG_OBJECT_TYPE: "Изменился тип объекта", INSPECTION_FAILED: "Не удалось проверить объект",
  IDENTITY_UNAVAILABLE: "Идентичность объекта недоступна", TIME_UNAVAILABLE: "Время изменения недоступно",
  TOO_RECENT_OR_FUTURE: "Свежий файл или время изменения в будущем", LIMIT: "Недоступен новый снимок исполнения",
};
export const scanReason = (code:string) => scanReasons[code] ?? "Объект исключён или данные недоступны";
