
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'app.main/main!) (:mode :native) (:reload-fn 'app.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'app.main
    %{} 'FileEntry
      :defs $ {}
        'helper-count $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn helper-count ()
            .count $ helper-list
          :examples $ []
        'helper-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn helper-list () ([] 1 2 3)
          :examples $ []
        'helper-number $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn helper-number () (+ 1 2)
          :examples $ []
        'helper-text $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn helper-text () (.slice "|😀中" 0 1)
          :examples $ []
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |closed-helper-contract)
            :code $ quote $ do
              assert= 3 $ helper-number
              assert= 6 $ .add (helper-number) (helper-count)
              assert= "|😀" $ helper-text
              assert= ([] 2 3 4)
                .map (helper-list)
                  fn (x) (inc x)
              assert= 3 $ .unwrap-or
                %some $ helper-number
                , 0
            :tags $ #{} :helper-inference :unit
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.main (:require)
