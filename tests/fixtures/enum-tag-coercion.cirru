{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :native) (:target :native)
      :modules $ []
  :files $ {}
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'Filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Filter (:all) (:active) (:done) (:tagged String)
          :schema $ quote EnumDef
        'Mode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Mode (:all) (:light) (:dark)
          :schema $ quote EnumDef
        'Choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Choice Filter Number
          :schema $ quote UnionDef
        'Either $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion Either Filter Mode
          :schema $ quote UnionDef
        'View $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct View (:filter Filter) (:title String)
          :schema $ quote StructDef
        'show-filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-filter (f)
            match f
              (:all) |all
              (:active) |active
              (:done) |done
              (:tagged t) t
          :schema $ quote $ Fn Filter (-> String)
        'show-mode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-mode (m)
            match m
              (:all) |every
              (:light) |light
              (:dark) |dark
          :schema $ quote $ Fn Mode (-> String)
        'show-choice $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-choice (c)
            match c
              (Filter f) (show-filter f)
              (Number n) (str n)
          :schema $ quote $ Fn Choice (-> String)
        'show-either $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-either (c)
            match c
              (Filter f) (show-filter f)
              (Mode m) (show-mode m)
          :schema $ quote $ Fn Either (-> String)
        'show-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-tag (t) (to-string t)
          :schema $ quote $ Fn Tag (-> String)
        'render-view $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn render-view (v)
            str (:title v) | (show-filter (:filter v))
          :schema $ quote $ Fn View (-> String)
        'default-filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn default-filter () :active
          :schema $ quote $ Fn (-> Filter)
        'pick-filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn pick-filter (done?)
            if done? :done :all
          :schema $ quote $ Fn Bool (-> Filter)
        'with-filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn with-filter (v) (assoc v :filter :done)
          :schema $ quote $ Fn View (-> View)
        'let-filter $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn let-filter (n)
            let
                a $ + n 1
              , :active
          :schema $ quote $ Fn Number (-> Filter)
        'flip-mode $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn flip-mode (m)
            match m
              (:all) :all
              (:light) :dark
              (:dark) :light
          :schema $ quote $ Fn Mode (-> Mode)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (show-filter :all) (show-filter (default-filter)) (show-filter (pick-filter true)) (show-choice :done) (show-choice 2) (show-tag :all) (show-either (Mode :all))
            println $ render-view $ %{} View (:filter :done) (:title |direct)
            println $ render-view $ {} (:filter :active) (:title |map)
            println $ str |tails (show-filter $ :filter $ with-filter $ %{} View (:filter :all) (:title |t)) (show-filter $ let-filter 1) (show-mode $ flip-mode $ Mode :light)
            let
                local $ fn (f)
                  hint-fn $ Fn Filter (-> String)
                  show-filter f
              println $ local :done
            , &unit
          :schema $ quote $ Fn (-> Unit)
