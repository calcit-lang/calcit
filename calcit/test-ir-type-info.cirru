
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |app.main $ %{} :FileEntry
      :defs $ {}
        |get-number $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn get-number () :number $ do 123
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () $ test-type-info
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn reload! () nil
          :examples $ []
        |test-type-info $ %{} :CodeEntry (:doc |) (:schema :dynamic)
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
