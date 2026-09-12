{}
  :schema-version 1
  :feature 'wasi-wait
  :doc "|提供单位明确、同步且跨 backend 一致的 wait-ms Result API；Preview 1 poll_oneoff 只作为 WASI command 的内部 adapter，timeout-call 保持独立。"
  :roots $ #{} 'calcit.core/wait-ms
  :definitions $ {}
    'calcit.core/&wait-ms $ {}
      :mode :ensure
      :kind :fn
      :doc "|内部同步等待边界；显式接收 Result 原型和宿主错误前缀，供 public wrapper 与 backend lowering 使用。"
      :params $ [] 'result-type 'milliseconds 'host-error
      :schema $ :: 'Fn
        {}
          :args $ [] 'EnumDef 'Number 'String
          :return $ :: 'Result 'Unit 'String
      :code $ quote &runtime-implementation
    'calcit.core/wait-ms $ {}
      :mode :ensure
      :kind :fn
      :doc "|同步等待整数毫秒并返回 Result<Unit,String>；允许 0..4294967295，不做隐式舍入。"
      :params $ [] 'milliseconds
      :schema $ :: 'Fn
        {}
          :args $ [] 'Number
          :return $ :: 'Result 'Unit 'String
      :code $ quote
        defn wait-ms (milliseconds)
          if
            and (round? milliseconds) (>= milliseconds 0) (<= milliseconds 4294967295)
            &wait-ms Result milliseconds "|wait-ms failed"
            %err "|wait-ms expected an integer millisecond duration in 0..4294967295"
  :edges $ #{}
    :: :call 'calcit.core/wait-ms 'calcit.core/&wait-ms
