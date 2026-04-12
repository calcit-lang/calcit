
{} (:about "|Machine-generated snapshot. AI AGENTS: never edit this file directly — changes will be overwritten on recompile. Inspect via `cr query`; modify via `cr edit` / `cr tree`. MANDATORY first step: run `cr docs agents --full`.") (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |get-number $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn get-number () :number $ do 123
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () $ test-type-info
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () nil
          :examples $ []
        |test-type-info $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn test-type-info () $ let
                x 123
              ; "这里" assert-type "会把类型信息加到" scope_types
              assert-type x :number
              ; "后续使用" x "时，Local" "节点会读取" scope_types "中的类型信息"
              let
                  y $ &+ x 1
                  z |hello
                  flag true
                  nothing nil
                  nums $ [] 1 2 3
                  result $ get-number
                  ; "测试嵌套表达式：内层" &let "的返回值类型"
                  nested $ let
                      inner 456
                    inner
                println y
                println z
                println flag
                println nothing
                println nums
                println result
                println nested
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote (ns app.main)
