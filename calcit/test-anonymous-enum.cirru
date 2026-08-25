
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --full` first. Manual edits must follow format and schema conventions, then run `calcit edit format`.") (:package |test-anonymous-enum)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-anonymous-enum.main/main!) (:mode :native) (:reload-fn 'test-anonymous-enum.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |test-anonymous-enum.main $ %{} 'FileEntry
      :defs $ {}
        |Result $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defenum Result (:ok 'Number) (:err 'String)
          :examples $ []
          :schema $ :: 'EnumDef
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing anonymous enum")
              assert= 1 $ try-size (:: :dyn)
              assert= 2 $ try-size (:: :dyn 1)
              assert= 3 $ try-size (:: :dyn 1 2)
              assert= 4 $ try-size (:: :dyn 1 2 3)
              assert= :many $ try-size (:: :dyn 1 2 3 4)
              assert= :many $ try-size (:: :dyn 1 2 3 4 5)
              let
                  ok $ %:: Result :ok 1
                assert= (%some Result) (enum-definition ok)
                assert= "|(%:: 'Result :ok 1)" $ str ok
                assert= true $ &enum-def:has-variant? Result :ok
                assert= 1 $ &enum-def:variant-arity Result :ok
                assert= &unit $ &enum:validate ok :ok
              let
                  plain $ :: :plain 1
                assert= (%none) (enum-definition plain)
          :examples $ []
          :schema $ :: 'Dynamic
        |try-size $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn try-size (x)
              tag-match x
                (:dyn) 1
                (:dyn x) 2
                (:dyn x y) 3
                (:dyn x y z) 4
                _ :many
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-anonymous-enum.main $ :require
            util.core :refer $ log-title
