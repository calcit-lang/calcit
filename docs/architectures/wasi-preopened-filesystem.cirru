{}
  :schema-version 1
  :feature 'wasi-preopened-filesystem
  :doc "|让 FsPath 的 Result API 直接经过类型化 runtime boundary；WASI lowering 在内部解析 host preopen，拒绝未授权绝对路径与越界路径，不向 Calcit 暴露 descriptor。第一切片覆盖 UTF-8 read/write，后续切片已接入即时 read-dir，递归 walk 继续复用同一边界。"
  :roots $ #{} 'calcit.core/fs-path:read-text 'calcit.core/fs-path:write-text
  :definitions $ {}
    'calcit.core/&fs-read-text $ {}
      :mode :ensure
      :kind :data
      :doc "|内部 UTF-8 文件读取边界；显式接收 Result 原型和宿主错误前缀，供 FsPath wrapper 与 backend lowering 使用。"
      :schema $ :: 'Fn
        {}
          :args $ [] 'EnumDef 'String 'String
          :return $ :: 'Result 'String 'String
      :code $ quote &runtime-implementation
    'calcit.core/&fs-write-text $ {}
      :mode :ensure
      :kind :data
      :doc "|内部 UTF-8 文件写入边界；显式接收 Result 原型和宿主错误前缀，供 FsPath wrapper 与 backend lowering 使用。"
      :schema $ :: 'Fn
        {}
          :args $ [] 'EnumDef 'String 'String 'String
          :return $ :: 'Result 'Unit 'String
      :code $ quote &runtime-implementation
    'calcit.core/fs-path:read-text $ {}
      :mode :ensure
      :kind :fn
      :doc "|读取 FsPath 指向的 UTF-8 文本，并以 Result<String,String> 返回。"
      :params $ [] 'self
      :schema $ :: 'Fn
        {}
          :args $ [] 'FsPath
          :return $ :: 'Result 'String 'String
      :code $ quote
        defn fs-path:read-text (self)
          &fs-read-text Result (:value self) "|fs-path:read-text failed"
    'calcit.core/fs-path:write-text $ {}
      :mode :ensure
      :kind :fn
      :doc "|把 UTF-8 文本写入 FsPath，并以 Result<Unit,String> 返回。"
      :params $ [] 'self 'content
      :schema $ :: 'Fn
        {}
          :args $ [] 'FsPath 'String
          :return $ :: 'Result 'Unit 'String
      :code $ quote
        defn fs-path:write-text (self content)
          &fs-write-text Result (:value self) content "|fs-path:write-text failed"
  :edges $ #{}
    :: :call 'calcit.core/fs-path:read-text 'calcit.core/&fs-read-text
    :: :call 'calcit.core/fs-path:write-text 'calcit.core/&fs-write-text
