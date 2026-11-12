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
        'Color $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Color (:red) (:green) (:rgb Number Number Number)
          :schema $ quote EnumDef
        'show-color $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-color (c)
            match c
              (:red) |red
              (:green) |green
              (:rgb r g b) |rgb
          :schema $ quote $ Fn Color (-> String)
        'show-colors $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn show-colors (xs) (join-string (map xs show-color) |,)
          :schema $ quote $ Fn (List Color) (-> String)
        'apply-num $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-num (f x) (f x)
          :schema $ quote $ Fn (Fn Number (-> Number)) Number (-> Number)
        'apply-all $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-all (fs x)
            foldl fs x $ fn (acc f) (f acc)
          :schema $ quote $ Fn (List (Fn Number (-> Number))) Number (-> Number)
        'lambda-if-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lambda-if-arg (c)
            apply-num
              if c (fn (x) (&+ x 1)) (fn (x) (&+ x 2))
              , 1
          :schema $ quote $ Fn Bool (-> Number)
        'lambda-let-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lambda-let-arg (n)
            apply-num
              &let (step n) (fn (x) (&+ x step))
              , 1
          :schema $ quote $ Fn Number (-> Number)
        'lambda-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lambda-list ()
            [] (fn (x) (&+ x 1)) (fn (x) (&* x 2))
          :schema $ quote $ Fn (-> (List (Fn Number (-> Number))))
        'lambda-map $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn lambda-map ()
            {} (:inc (fn (x) (&+ x 1)))
          :schema $ quote $ Fn (-> (Map Tag (Fn Number (-> Number))))
        'tag-if-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-if-arg (c) (show-color (if c :red :green))
          :schema $ quote $ Fn Bool (-> String)
        'tag-list-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-list-arg () (show-colors ([] :red :green))
          :schema $ quote $ Fn (-> String)
        'tag-list-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-list-return () ([] :green :red)
          :schema $ quote $ Fn (-> (List Color))
        'tag-set-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-set-return () (#{} :green)
          :schema $ quote $ Fn (-> (Set Color))
        'tag-map-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-map-return () ({} (|a :red))
          :schema $ quote $ Fn (-> (Map String Color))
        'plain-tags $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn plain-tags () ([] :red :other)
          :schema $ quote $ Fn (-> (List Tag))
        'Wrap $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum Wrap (:one Color) (:zero)
          :schema $ quote EnumDef
        'tag-eq $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-eq (c) (= c :red)
          :schema $ quote $ Fn Color (-> Bool)
        'tag-some-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-some-return () (Some :red)
          :schema $ quote $ Fn (-> (Option Color))
        'tag-qualified-option $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-qualified-option () (calcit.core/Option :some :green)
          :schema $ quote $ Fn (-> (Option Color))
        'tag-payload $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-payload () (Wrap :one :green)
          :schema $ quote $ Fn (-> Wrap)
        'tag-match-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-match-arg (w)
            show-color $ match w
              (:one c) c
              (:zero) :red
          :schema $ quote $ Fn Wrap (-> String)
        'tag-match-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-match-return (w)
            match w
              (:one c) c
              (:zero) :green
          :schema $ quote $ Fn Wrap (-> Color)
        'NumOrStr $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defunion NumOrStr Number String
          :schema $ quote UnionDef
        'tag-literal-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-literal-match (n)
            match n
              0 :red
              _ :green
          :schema $ quote $ Fn Number (-> Color)
        'tag-union-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-union-match (v)
            match v
              (Number n) :red
              (String s) :green
          :schema $ quote $ Fn NumOrStr (-> Color)
        'tag-data-view-match $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-data-view-match (x)
            match (data-view x)
              (:string s) :red
              _ :green
          :schema $ quote $ Fn Dynamic (-> Color)
        'match-scrutinee-keeps-tag $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn match-scrutinee-keeps-tag ()
            match (Some :sentinel)
              (:some t) (Some :red)
              (:none) (Option :none)
          :schema $ quote $ Fn (-> (Option Color))
        'Task $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Task (:title String) (:tags (List Color))
          :schema $ quote StructDef
        'tag-method-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-method-arg (o) (.unwrap-or o :red)
          :schema $ quote $ Fn (Option Color) (-> Color)
        'tag-local-fn-arg $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-local-fn-arg ()
            let
                f $ fn (c)
                  hint-fn $ Fn Color (-> String)
                  show-color c
              f :green
          :schema $ quote $ Fn (-> String)
        'tag-tuple-return $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-tuple-return () ([] :green 1)
          :schema $ quote $ Fn (-> (Tuple Color Number))
        'tag-tuple-list $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-tuple-list () ([] ([] :red 1) ([] :green 2))
          :schema $ quote $ Fn (-> (List (Tuple Color Number)))
        'tag-assoc-field $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn tag-assoc-field (t) (assoc t :tags ([] :red :red))
          :schema $ quote $ Fn Task (-> Task)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println (lambda-if-arg true) (lambda-if-arg false) (lambda-let-arg 5)
            println (apply-all (lambda-list) 1)
            println (tag-if-arg true) (tag-if-arg false) (tag-list-arg)
            println (show-colors (tag-list-return)) (count (tag-set-return)) (count (plain-tags))
            println (tag-eq (Color :red)) (tag-eq (Color :green)) (tag-match-arg (tag-payload)) (tag-match-arg (Wrap :zero))
            println (show-color (tag-match-return (Wrap :zero))) (count ([] (tag-some-return)))
            println (show-color (tag-literal-match 0)) (show-color (tag-literal-match 1)) (show-color (tag-union-match |s)) (count ([] (match-scrutinee-keeps-tag)))
            println (show-color (tag-data-view-match |s)) (show-color (tag-data-view-match 1))
            println (show-color (tag-method-arg (Option :none))) (tag-local-fn-arg) (show-color (nth (tag-tuple-return) 0)) (count (tag-tuple-list))
            println $ show-colors (:tags (tag-assoc-field (Task :title |t :tags ([]))))
          :schema $ quote $ Fn (-> Unit) (:features :io)
