{}
  :schema-version 1
  :feature 'wasi-read-dir
  :doc "|让 FsPath 的即时子目录枚举复用 preopen capability boundary；Preview 1 的 descriptor、cookie 分页、截断记录与 UTF-8 校验全部封装在 WASI adapter 内，表层只观察确定排序的 Result<List<FsPath>,String>。"
  :roots $ #{} 'calcit.core/fs-path:read-dir
  :definitions $ {}
    'calcit.core/&fs-read-dir $ {}
      :mode :ensure
      :kind :fn
      :doc "|内部目录枚举边界；显式接收 Result 与 FsPath 定义、guest path 和宿主错误前缀。"
      :params $ [] 'result-type 'path-type 'path 'host-error
      :schema $ :: 'Fn
        {}
          :args $ [] 'EnumDef 'StructDef 'String 'String
          :return $ :: 'Result (:: 'List 'FsPath) 'String
      :code $ quote &runtime-implementation
    'calcit.core/fs-path:read-dir $ {}
      :mode :ensure
      :kind :fn
      :doc "|枚举 FsPath 的即时子项，并以确定顺序返回 Result<List<FsPath>,String>。"
      :params $ [] 'self
      :schema $ :: 'Fn
        {}
          :args $ [] 'FsPath
          :return $ :: 'Result (:: 'List 'FsPath) 'String
      :code $ quote
        defn fs-path:read-dir (self)
          &fs-read-dir Result FsPath (:value self) "|fs-path:read-dir failed"
  :edges $ #{}
    :: :call 'calcit.core/fs-path:read-dir 'calcit.core/&fs-read-dir
