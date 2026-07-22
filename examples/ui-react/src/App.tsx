import { useState, useEffect } from "react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { Checkbox } from "@/components/ui/checkbox"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog"
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert"
import { Progress } from "@/components/ui/progress"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Skeleton } from "@/components/ui/skeleton"
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card"
import { Separator } from "@/components/ui/separator"

function ButtonShowcase() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Button & Badge System</CardTitle>
        <CardDescription>Standard variants and sizes ported from shadcn/ui.</CardDescription>
      </CardHeader>
      <CardContent>
        {/* Variants */}
        <div className="flex flex-wrap items-center gap-2 mb-6">
          <span className="text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1">
            Variants
          </span>
          <Button variant="default">Default</Button>
          <Button variant="destructive">Destructive</Button>
          <Button variant="outline">Outline</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="link">Link</Button>
        </div>

        {/* Sizes */}
        <div className="flex flex-wrap items-center gap-2 mb-6">
          <span className="text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1">
            Sizes
          </span>
          <Button variant="outline" size="xs">XS</Button>
          <Button variant="outline" size="sm">Small</Button>
          <Button variant="outline" size="default">Default</Button>
          <Button variant="outline" size="lg">Large</Button>
          <Button variant="outline" size="icon">★</Button>
          <Button variant="default" size="icon-sm">⚡</Button>
        </div>

        {/* Badges */}
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs font-semibold text-slate-500 dark:text-slate-400 w-full mb-1">
            Badges
          </span>
          <Badge variant="default">Default</Badge>
          <Badge variant="secondary">Secondary</Badge>
          <Badge variant="destructive">Destructive</Badge>
          <Badge variant="outline">Outline</Badge>
          <Badge variant="ghost">Ghost</Badge>
          <Badge variant="link">Link</Badge>
        </div>
      </CardContent>
    </Card>
  )
}

function FormControlsShowcase() {
  const [textVal, setTextVal] = useState("Hello Silex UI!")
  const [checkedVal, setCheckedVal] = useState(true)
  const [switchVal, setSwitchVal] = useState(true)

  return (
    <Card>
      <CardHeader>
        <CardTitle>Form & Interactive Controls</CardTitle>
        <CardDescription>Reactive Input, Textarea, Checkbox and Switch components.</CardDescription>
      </CardHeader>
      <CardContent>
        {/* Input */}
        <div className="flex flex-col mb-6">
          <span className="text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1">
            Text Input
          </span>
          <Input
            value={textVal}
            placeholder="Type something..."
            onChange={(e) => setTextVal(e.target.value)}
          />
          <p className="text-xs text-slate-500 mt-1.5 font-mono">
            Live Bound Value: '{textVal}'
          </p>
        </div>

        {/* Textarea */}
        <div className="flex flex-col mb-6">
          <span className="text-xs font-semibold text-slate-500 dark:text-slate-400 mb-1">
            Textarea
          </span>
          <Textarea
            defaultValue="Multi-line textarea component styling ported straight from shadcn/ui v4."
            placeholder="Write a description..."
          />
        </div>

        {/* Checkbox & Switch */}
        <div className="flex flex-wrap items-center justify-between gap-4 p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800">
          <div className="flex items-center gap-2">
            <Checkbox
              checked={checkedVal}
              onCheckedChange={(checked: boolean | "indeterminate") => setCheckedVal(Boolean(checked))}
            />
            <span className="text-sm font-medium text-slate-900 dark:text-slate-100">
              Enable Notifications
            </span>
          </div>

          <div className="flex items-center gap-2">
            <Switch
              checked={switchVal}
              onCheckedChange={(checked: boolean) => setSwitchVal(Boolean(checked))}
            />
            <span className="text-sm font-medium text-slate-900 dark:text-slate-100">
              Airplane Mode
            </span>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function TabsAndDialogShowcase() {
  const [activeTab, setActiveTab] = useState("account")
  const [dialogOpen, setDialogOpen] = useState(false)

  return (
    <Card>
      <CardHeader>
        <CardTitle>Tabs & Modal Dialog</CardTitle>
        <CardDescription>Seamless tab switching and portal-rendered modal dialogs.</CardDescription>
      </CardHeader>
      <CardContent>
        {/* Tabs */}
        <div className="mb-6">
          <Tabs value={activeTab} onValueChange={setActiveTab}>
            <TabsList className="grid w-full grid-cols-3">
              <TabsTrigger value="account">Account</TabsTrigger>
              <TabsTrigger value="password">Password</TabsTrigger>
              <TabsTrigger value="settings">Settings</TabsTrigger>
            </TabsList>
            <TabsContent value="account">
              <p className="p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300">
                Manage your account details and profile preferences.
              </p>
            </TabsContent>
            <TabsContent value="password">
              <p className="p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300">
                Change your password and configure 2FA security.
              </p>
            </TabsContent>
            <TabsContent value="settings">
              <p className="p-4 rounded-lg bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-800 text-sm text-slate-700 dark:text-slate-300">
                Customize system settings and notification channels.
              </p>
            </TabsContent>
          </Tabs>
        </div>

        <Separator className="my-4" />

        <div className="flex items-center text-xs font-medium text-slate-500 dark:text-slate-400 mb-4">
          <span>Silex UI</span>
          <Separator orientation="vertical" className="h-4 mx-2" />
          <span>Docs</span>
          <Separator orientation="vertical" className="h-4 mx-2" />
          <span>GitHub</span>
        </div>

        {/* Dialog Trigger */}
        <div className="flex items-center justify-between">
          <Button variant="default" onClick={() => setDialogOpen(true)}>
            Open Modal Dialog
          </Button>

          <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
            <DialogContent>
              <DialogHeader>
                <DialogTitle>Edit Profile</DialogTitle>
                <DialogDescription>
                  Make changes to your profile here. Click save when you're done.
                </DialogDescription>
              </DialogHeader>
              <div className="grid gap-3 py-4">
                <Input defaultValue="Shao G." placeholder="Name" />
                <Input defaultValue="shaog.me@gmail.com" placeholder="Email" />
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={() => setDialogOpen(false)}>
                  Cancel
                </Button>
                <Button variant="default" onClick={() => setDialogOpen(false)}>
                  Save Changes
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
      </CardContent>
    </Card>
  )
}

function FeedbackAndDataShowcase() {
  const [progressVal, setProgressVal] = useState(45)

  return (
    <Card>
      <CardHeader>
        <CardTitle>Avatars, Progress & Feedback</CardTitle>
        <CardDescription>
          Progress indicators, Avatar fallback, Alert banners and Skeletons.
        </CardDescription>
      </CardHeader>
      <CardContent>
        {/* Alert */}
        <Alert variant="default" className="mb-6">
          <AlertTitle>System Update Complete</AlertTitle>
          <AlertDescription>
            All shadcn/ui components have been successfully installed via official CLI into React & Tailwind v4.
          </AlertDescription>
        </Alert>

        {/* Progress Bar */}
        <div className="mb-6">
          <div className="flex justify-between items-center mb-1.5">
            <span className="text-xs font-semibold text-slate-500 dark:text-slate-400">
              Progress
            </span>
            <span className="text-xs font-bold text-indigo-600 dark:text-indigo-400">
              {progressVal}%
            </span>
          </div>
          <Progress value={progressVal} className="mb-3" />
          <div className="flex justify-end gap-2">
            <Button
              variant="outline"
              size="xs"
              onClick={() => setProgressVal((v) => Math.max(0, v - 10))}
            >
              -10%
            </Button>
            <Button
              variant="outline"
              size="xs"
              onClick={() => setProgressVal((v) => Math.min(100, v + 10))}
            >
              +10%
            </Button>
          </div>
        </div>

        {/* Avatar & Skeleton */}
        <div className="flex items-center justify-between p-4 bg-slate-50 dark:bg-slate-900 rounded-lg border border-solid border-slate-200 dark:border-slate-800">
          {/* Avatars */}
          <div className="flex items-center gap-3">
            <Avatar>
              <AvatarFallback>SG</AvatarFallback>
            </Avatar>
            <Avatar className="bg-indigo-600 text-white">
              <AvatarFallback className="bg-indigo-600 text-white">UI</AvatarFallback>
            </Avatar>
            <Avatar className="bg-emerald-600 text-white">
              <AvatarFallback className="bg-emerald-600 text-white">SX</AvatarFallback>
            </Avatar>
          </div>

          {/* Skeletons */}
          <div className="flex flex-col gap-2">
            <Skeleton className="h-4 w-32" />
            <Skeleton className="h-4 w-20" />
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export default function App() {
  const [isDark, setIsDark] = useState(() => {
    const saved = localStorage.getItem("silex-ui-dark")
    return saved !== null ? JSON.parse(saved) : true
  })

  useEffect(() => {
    localStorage.setItem("silex-ui-dark", JSON.stringify(isDark))
    if (isDark) {
      document.documentElement.classList.add("dark")
    } else {
      document.documentElement.classList.remove("dark")
    }
  }, [isDark])

  return (
    <div className={`min-h-screen p-4 sm:p-8 transition-colors duration-300 bg-slate-100 text-slate-900 dark:bg-slate-900 dark:text-slate-50 ${isDark ? "dark" : ""}`}>
      <div>
        {/* Header */}
        <div className="w-full flex items-center justify-between mb-8 max-w-6xl mx-auto">
          <div className="flex items-center gap-3">
            <span className="text-xs font-black uppercase tracking-widest px-3.5 py-1.5 bg-indigo-50 dark:bg-indigo-950/80 text-indigo-600 dark:text-indigo-400 rounded-full border border-solid border-indigo-200 dark:border-indigo-800/60 shadow-sm">
              🎨 Silex UI Kit
            </span>
            <span className="hidden sm:inline-block text-xs font-semibold text-slate-500 dark:text-slate-400">
              shadcn/ui v4 Ported Component Suite
            </span>
          </div>

          <button
            onClick={() => setIsDark((d: boolean) => !d)}
            className="flex items-center gap-2 px-4 py-2 bg-slate-100 text-slate-800 dark:bg-slate-800 dark:text-amber-300 font-bold text-xs rounded-full cursor-pointer border border-solid border-slate-300 dark:border-slate-700 transition-all duration-300 hover:scale-105 shadow-sm"
          >
            {isDark ? "🌙 Dark Mode" : "☀️ Light Mode"}
          </button>
        </div>

        {/* Hero */}
        <div className="flex flex-col items-center max-w-6xl mx-auto mb-10">
          <h1 className="text-3xl sm:text-5xl font-black text-slate-900 dark:text-white tracking-tight mb-4 text-center">
            Pure Rust shadcn/ui Component Library
          </h1>
          <p className="text-sm sm:text-base text-slate-600 dark:text-slate-300 max-w-2xl text-center leading-relaxed mb-8">
            Zero-runtime overhead Tailwind CSS styling with fine-grained signal reactivity and type-safe Rust components.
          </p>
        </div>

        {/* Masonry Component Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6 items-start w-full max-w-6xl mx-auto">
          <div className="flex flex-col gap-6 w-full">
            <ButtonShowcase />
            <FormControlsShowcase />
          </div>

          <div className="flex flex-col gap-6 w-full">
            <TabsAndDialogShowcase />
            <FeedbackAndDataShowcase />
          </div>
        </div>
      </div>
    </div>
  )
}
