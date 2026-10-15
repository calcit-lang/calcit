
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'app.replay.browser/answer) (:mode :native) (:reload-fn 'app.replay.browser/answer) (:target :browser)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :node $ {} (:description |) (:init-fn 'app.replay.node/answer) (:mode :js) (:reload-fn 'app.replay.node/answer) (:target :node)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.replay.browser $ %{} 'FileEntry
      :defs $ {} $ 'answer
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn answer () 3
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
          :tests $ [] $ %{} 'TestEntry (:name |uses-the-selected-entry-target)
            :code $ quote $ assert= 3 (answer)
            :tags $ #{} :unit
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.replay.browser
    'app.replay.node $ %{} 'FileEntry
      :defs $ {} $ 'answer
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn answer () 0
          :examples $ []
          :ffi $ {} (:target :node)
            :js $ {} $ :inline "|() => 7"
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ []
            :features $ #{} :js-ffi
          :tests $ [] $ %{} 'TestEntry (:name |uses-the-selected-entry-target)
            :code $ quote $ assert= 7 (answer)
            :tags $ #{} :js :node
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.replay.node
