
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |narrowed
  :entries $ {} $ :default
    {} (:description "|0.35.0 收紧的 core 返回类型 fixture（#2549、#2556）") (:init-fn 'narrowed.core/main!) (:mode :native) (:reload-fn 'narrowed.core/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'narrowed.core
    %{} 'FileEntry
      :defs $ {}
        'count-rows $ %{} 'CodeEntry (:doc "|接受任意列表。")
          :code $ quote $ defn count-rows (xs) (count xs)
          :examples $ []
          :schema $ quote $ Fn (List Dynamic) (-> Number)
        'main! $ %{} 'CodeEntry (:doc "|把 diff-triple 结果整体传给 List 参数从 0.35.0 起报错；按位置读取和 Cirru 行列表传给 List Dynamic 仍然合法。")
          :code $ quote $ defn main! ()
            let
                triple $ &map:diff-triple ({} (|a 1)) ({} (|b 2))
                rows $ parse-cirru-list "|a b"
              println (count-rows triple) (&list:nth triple 0) (count triple) (count-rows rows)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns narrowed.core
